pub use smithay_client_toolkit::reexports::client::{EventQueue, Main};
pub use smithay_client_toolkit::reexports::protocols::wlr::unstable::layer_shell::v1::client::{
    zwlr_layer_shell_v1 as layer_shell, zwlr_layer_surface_v1 as layer_surface,
};
use smithay_client_toolkit::{
    default_environment,
    environment::{Environment, SimpleGlobal},
    get_surface_scale_factor, init_default_environment,
    reexports::{
        client::protocol::{wl_pointer, wl_surface}, //{wl_display, wl_keyboard, wl_output, wl_shm, },
        client::{Attached, ConnectError, Display, Proxy},
    },
};

use iced_graphics::window::Compositor;
use iced_native::{mouse, Cache, Command, Size, UserInterface};
use iced_wgpu::window::Compositor as WgpuCompositor;

use futures::{channel::mpsc, prelude::*};
use std::sync::Arc;

use crate::{event_loop::*, handle::*};

default_environment!(Env,
    fields = [
        layer_shell: SimpleGlobal<layer_shell::ZwlrLayerShellV1>,
    ],
    singles = [
        layer_shell::ZwlrLayerShellV1 => layer_shell
    ],
);

pub fn make_env() -> Result<(Environment<Env>, Display, EventQueue), ConnectError> {
    init_default_environment!(Env, fields = [layer_shell: SimpleGlobal::new(),])
}

static mut SCALE_CHANNELS: Vec<(wl_surface::WlSurface, mpsc::UnboundedSender<i32>)> = Vec::new();

pub trait DesktopWidget {
    fn setup_lsh(&self, layer_surface: &Main<layer_surface::ZwlrLayerSurfaceV1>);
}

pub type Element<'a, Message> = iced_native::Element<'a, Message, iced_wgpu::Renderer>;

pub trait IcedWidget {
    type Message: std::fmt::Debug + Send;

    fn view(&mut self) -> Element<'_, Self::Message>;
    fn update(&mut self, message: Self::Message) -> Command<Self::Message>;
}

pub struct IcedInstance<T> {
    widget: T,

    // wayland handles
    env: Environment<Env>,
    // display: Display,
    wl_surface: Attached<wl_surface::WlSurface>,
    layer_surface: Main<layer_surface::ZwlrLayerSurfaceV1>,
    scale_rx: mpsc::UnboundedReceiver<i32>,

    // wayland state
    ptr_active: bool,
    scale: i32,

    // iced render state
    cache: Cache,
    size: Size,
    compositor: WgpuCompositor,
    renderer: <WgpuCompositor as Compositor>::Renderer,
    gpu_surface: <WgpuCompositor as Compositor>::Surface,
    swap_chain: Option<<WgpuCompositor as Compositor>::SwapChain>,
}

impl<T: DesktopWidget + IcedWidget> IcedInstance<T> {
    pub async fn new(
        widget: T,
        env: Environment<Env>,
        display: Display,
        queue: &EventQueue,
    ) -> IcedInstance<T> {
        let layer_shell = env.require_global::<layer_shell::ZwlrLayerShellV1>();

        let (scale_tx, scale_rx) = mpsc::unbounded();
        let wl_surface: Proxy<wl_surface::WlSurface> = env
            .create_surface_with_scale_callback(|scale, wlsurf, _dd| unsafe {
                SCALE_CHANNELS
                    .iter()
                    .find(|(surf, _)| *surf == wlsurf)
                    .unwrap()
                    .1
                    .unbounded_send(scale)
                    .unwrap();
            })
            .into();
        let wl_surface = wl_surface.attach(queue.token());
        unsafe {
            SCALE_CHANNELS.push((wl_surface.detach(), scale_tx));
        }

        let layer_surface = layer_shell.get_layer_surface(
            &wl_surface,
            None,
            layer_shell::Layer::Top,
            "Waysmoke Surface".to_owned(),
        );
        widget.setup_lsh(&layer_surface);

        let mut compositor = WgpuCompositor::request(iced_wgpu::Settings {
            background_color: iced_core::Color::TRANSPARENT,
            ..iced_wgpu::Settings::default()
        })
        .await
        .unwrap();
        let renderer = iced_wgpu::Renderer::new(compositor.create_backend());
        let gpu_surface =
            compositor.create_surface(&ToRWH((*wl_surface.as_ref()).clone(), (*display).clone()));
        wl_surface.commit();
        display.flush().unwrap();

        IcedInstance {
            widget,
            env,
            // display,
            wl_surface,
            layer_surface,
            scale_rx,
            ptr_active: false,
            scale: 1,
            cache: Cache::new(),
            size: Size::new(0.0, 0.0),
            compositor,
            renderer,
            gpu_surface,
            swap_chain: None,
        }
    }

    async fn render(&mut self, mut events: Vec<iced_native::Event>) {
        let swap_chain = self.swap_chain.as_mut().unwrap();

        let mut user_interface = UserInterface::build(
            self.widget.view(),
            self.size,
            self.cache.clone(),
            &mut self.renderer,
        );
        let messages = user_interface.update(events.drain(..), None, &mut self.renderer);
        let viewport = iced_graphics::Viewport::with_physical_size(
            iced_graphics::Size::new(
                self.size.width as u32 * self.scale as u32,
                self.size.height as u32 * self.scale as u32,
            ),
            self.scale as _,
        );

        if messages.is_empty() {
            let primitive = user_interface.draw(&mut self.renderer);
            let _new_mouse_cursor = self.compositor.draw::<String>(
                &mut self.renderer,
                swap_chain,
                &viewport,
                &primitive,
                &[],
            );
            self.cache = user_interface.into_cache();
        } else {
            // iced-winit says we are forced to rebuild twice
            let temp_cache = user_interface.into_cache();

            for message in messages {
                for f in self.widget.update(message).futures() {
                    f.await;
                }
            }

            let user_interface = UserInterface::build(
                self.widget.view(),
                self.size,
                temp_cache,
                &mut self.renderer,
            );
            let primitive = user_interface.draw(&mut self.renderer);
            let _new_mouse_cursor = self.compositor.draw::<String>(
                &mut self.renderer,
                swap_chain,
                &viewport,
                &primitive,
                &[],
            );
            self.cache = user_interface.into_cache();
        }
    }

    fn create_swap_chain(&mut self) {
        self.swap_chain = Some(self.compositor.create_swap_chain(
            &self.gpu_surface,
            self.size.width as u32 * self.scale as u32,
            self.size.height as u32 * self.scale as u32,
        ));
        self.wl_surface.set_buffer_scale(self.scale);
    }

    async fn on_scale(&mut self, scale: i32) {
        if scale == self.scale {
            return;
        }
        self.scale = scale;
        self.create_swap_chain();
        self.render(vec![]).await;
    }

    async fn on_layer_event(&mut self, event: Arc<layer_surface::Event>) {
        match &*event {
            layer_surface::Event::Configure {
                ref serial,
                ref width,
                ref height,
            } => {
                self.layer_surface.ack_configure(*serial);

                self.scale = get_surface_scale_factor(&self.wl_surface);
                self.size = Size::new((*width) as f32, (*height) as f32);
                self.create_swap_chain();
                self.render(vec![]).await;
            }
            _ => eprintln!("todo: lsh close"),
        }
    }

    async fn on_pointer_event(&mut self, event: Arc<wl_pointer::Event>) {
        match &*event {
            wl_pointer::Event::Enter { surface, .. } => {
                if self.wl_surface.detach() == *surface {
                    self.ptr_active = true;
                }
            }
            wl_pointer::Event::Leave { surface, .. } => {
                if self.wl_surface.detach() == *surface {
                    self.ptr_active = false;
                }
            }
            wl_pointer::Event::Button { button, state, .. } => {
                if self.ptr_active {
                    let btn = match *button {
                        0x110 => mouse::Button::Left,
                        0x111 => mouse::Button::Right,
                        0x112 => mouse::Button::Middle,
                        x if x > 0x110 => mouse::Button::Other((x - 0x110) as u8),
                        _ => panic!("low button event code"),
                    };
                    self.render(vec![iced_native::Event::Mouse(match state {
                        wl_pointer::ButtonState::Pressed => mouse::Event::ButtonPressed(btn),
                        wl_pointer::ButtonState::Released => mouse::Event::ButtonReleased(btn),
                        _ => panic!("new button state?"),
                    })])
                    .await;
                }
            }
            wl_pointer::Event::Motion {
                surface_x,
                surface_y,
                ..
            } => {
                if self.ptr_active {
                    self.render(vec![iced_native::Event::Mouse(mouse::Event::CursorMoved {
                        x: *surface_x as _,
                        y: *surface_y as _,
                    })])
                    .await;

                    // frame_cb = Some(wl_surface.frame());
                }
            }
            wl_pointer::Event::Frame { .. } => {
                // TODO use this
            }
            _ => {
                eprintln!("unhandled pointer event");
            }
        }
    }

    pub async fn run(&mut self) {
        let seat = &self.env.get_all_seats()[0];
        let mut ptr_events = wayland_event_chan(&seat.get_pointer());
        let mut layer_events = wayland_event_chan(&self.layer_surface);

        loop {
            futures::select! {
                ev = layer_events.next() => if let Some(event) = ev { self.on_layer_event(event).await },
                ev = ptr_events.next() => if let Some(event) = ev { self.on_pointer_event(event).await },
                sc = self.scale_rx.next() => if let Some(scale) = sc { self.on_scale(scale).await },
            }
        }
    }
}
