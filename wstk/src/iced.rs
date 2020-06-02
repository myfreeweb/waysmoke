use smithay_client_toolkit::{
    default_environment, get_surface_scale_factor, init_default_environment,
};
pub use smithay_client_toolkit::{
    environment::{Environment, SimpleGlobal},
    reexports::{
        client::{
            protocol::{wl_compositor, wl_pointer, wl_surface},
            Attached, ConnectError, Display, EventQueue, Main, Proxy,
        },
        protocols::wlr::unstable::layer_shell::v1::client::{
            zwlr_layer_shell_v1 as layer_shell, zwlr_layer_surface_v1 as layer_surface,
        },
    },
};

use iced_graphics::window::Compositor;
pub use iced_native::Rectangle;
use iced_native::{mouse, Cache, Damage, Size, UserInterface};
use iced_wgpu::window::Compositor as WgpuCompositor;

use std::{marker::Unpin, pin::Pin, sync::Arc};

use futures::channel::mpsc;
pub use futures::prelude::*;

pub use async_trait::async_trait;

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

#[async_trait]
pub trait IcedWidget {
    type Message: std::fmt::Debug + Send;
    type ExternalEvent: std::fmt::Debug + Send;

    fn view(&mut self) -> Element<'_, Self::Message>;
    fn input_region(&self, width: i32, height: i32) -> Option<Vec<Rectangle<i32>>>;

    async fn update(&mut self, message: Self::Message);
    async fn react(&mut self, event: Self::ExternalEvent);
    // TODO: ExternalEvent | IcedEvent | LshEvent

    async fn on_pointer_enter(&mut self) {}
    async fn on_pointer_leave(&mut self) {}
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
    leave_timeout: Option<future::Fuse<Pin<Box<dyn Future<Output = ()> + Send + 'static>>>>,
    prev_input_region: Option<Vec<Rectangle<i32>>>,

    // iced render state
    cache: Cache,
    size: Size,
    compositor: WgpuCompositor,
    renderer: <WgpuCompositor as Compositor>::Renderer,
    gpu_surface: <WgpuCompositor as Compositor>::Surface,
    swap_chain: Option<<WgpuCompositor as Compositor>::SwapChain>,
    prev_prim: iced_graphics::Primitive,
    queue: Vec<iced_native::Event>,
}

impl<T: DesktopWidget + IcedWidget + Send> IcedInstance<T> {
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
            leave_timeout: None,
            prev_input_region: None,
            cache: Cache::new(),
            size: Size::new(0.0, 0.0),
            compositor,
            renderer,
            gpu_surface,
            swap_chain: None,
            prev_prim: iced_graphics::Primitive::None,
            queue: Vec::new(),
        }
    }

    async fn render(&mut self) {
        let reg = self
            .widget
            .input_region(self.size.width as i32, self.size.height as i32);
        if reg != self.prev_input_region {
            if let Some(ref rects) = reg {
                let wlreg = self
                    .env
                    .require_global::<wl_compositor::WlCompositor>()
                    .create_region();
                for rect in rects.iter() {
                    wlreg.add(rect.x, rect.y, rect.width, rect.height);
                }
                self.wl_surface.set_input_region(Some(&wlreg.detach()));
            } else {
                self.wl_surface.set_input_region(None);
            }
        }
        self.prev_input_region = reg;

        let swap_chain = self.swap_chain.as_mut().unwrap();

        let mut user_interface = UserInterface::build(
            self.widget.view(),
            self.size,
            self.cache.clone(),
            &mut self.renderer,
        );
        let messages = user_interface.update(self.queue.drain(..), None, &mut self.renderer);
        let viewport = iced_graphics::Viewport::with_physical_size(
            iced_graphics::Size::new(
                self.size.width as u32 * self.scale as u32,
                self.size.height as u32 * self.scale as u32,
            ),
            self.scale as _,
        );

        if messages.is_empty() {
            let (primitive, mi) = user_interface.draw(&mut self.renderer);
            let dmg = self.prev_prim.damage(&primitive);
            self.prev_prim = primitive.clone();
            if dmg == None || dmg.map(|x| x.len()).unwrap_or(0) == 0 {
                self.cache = user_interface.into_cache();
                return;
            }
            let _new_mouse_cursor = self.compositor.draw::<String>(
                &mut self.renderer,
                swap_chain,
                &viewport,
                &(primitive, mi),
                &[],
            );
            self.cache = user_interface.into_cache();
        } else {
            // iced-winit says we are forced to rebuild twice
            let temp_cache = user_interface.into_cache();

            for message in messages {
                self.widget.update(message).await;
            }

            let user_interface = UserInterface::build(
                self.widget.view(),
                self.size,
                temp_cache,
                &mut self.renderer,
            );
            let (primitive, mi) = user_interface.draw(&mut self.renderer);
            let dmg = self.prev_prim.damage(&primitive);
            self.prev_prim = primitive.clone();
            if dmg == None || dmg.map(|x| x.len()).unwrap_or(0) == 0 {
                self.cache = user_interface.into_cache();
                return;
            }
            let _new_mouse_cursor = self.compositor.draw::<String>(
                &mut self.renderer,
                swap_chain,
                &viewport,
                &(primitive, mi),
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
        self.render().await;
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
                self.render().await;
            }
            _ => eprintln!("todo: lsh close"),
        }
    }

    async fn on_pointer_event(&mut self, event: Arc<wl_pointer::Event>) {
        match &*event {
            wl_pointer::Event::Enter { surface, .. } => {
                if self.wl_surface.detach() == *surface {
                    self.ptr_active = true;
                    self.leave_timeout = None;
                    self.widget.on_pointer_enter().await;
                }
            }
            wl_pointer::Event::Leave { surface, .. } => {
                if self.wl_surface.detach() == *surface {
                    self.ptr_active = false;
                    self.leave_timeout = Some(glib::timeout_future(420).fuse());
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
                    self.queue.push(iced_native::Event::Mouse(match state {
                        wl_pointer::ButtonState::Pressed => mouse::Event::ButtonPressed(btn),
                        wl_pointer::ButtonState::Released => mouse::Event::ButtonReleased(btn),
                        _ => panic!("new button state?"),
                    }));
                }
            }
            wl_pointer::Event::Motion {
                surface_x,
                surface_y,
                ..
            } => {
                if self.ptr_active {
                    self.queue
                        .push(iced_native::Event::Mouse(mouse::Event::CursorMoved {
                            x: *surface_x as _,
                            y: *surface_y as _,
                        }));
                }
            }
            wl_pointer::Event::Frame { .. } => {
                self.render().await;
            }
            _ => {
                eprintln!("unhandled pointer event");
            }
        }
    }

    pub async fn run(&mut self, ext_evt_src: &mut (impl Stream<Item = T::ExternalEvent> + Unpin)) {
        let seat = &self.env.get_all_seats()[0];
        let mut ptr_events = wayland_event_chan(&seat.get_pointer());
        let mut layer_events = wayland_event_chan(&self.layer_surface);

        loop {
            let leave_timeout_existed = self.leave_timeout.is_some();
            let mut leave_timeout = self
                .leave_timeout
                .take()
                .unwrap_or_else(|| future::pending::<()>().boxed().fuse());
            // allocation of the pending ^^^ >_< why doesn't select work well with maybe-not-existing futures
            futures::select! {
                ev = layer_events.next() => if let Some(event) = ev { self.on_layer_event(event).await },
                ev = ptr_events.next() => if let Some(event) = ev { self.on_pointer_event(event).await },
                sc = self.scale_rx.next() => if let Some(scale) = sc { self.on_scale(scale).await },
                ev = ext_evt_src.next().fuse() => if let Some(event) = ev {
                    self.widget.react(event).await;
                    self.render().await
                },
                () = leave_timeout => {
                    self.widget.on_pointer_leave().await;
                    // not getting a pointer frame after the timeout ;)
                    self.render().await;
                },
            }
            if leave_timeout_existed && !self.ptr_active {
                self.leave_timeout = Some(leave_timeout);
            }
        }
    }
}
