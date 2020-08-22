use iced_graphics::window::Compositor;
pub use iced_native::Rectangle;
use iced_native::{mouse, Cache, Damage, Point, Size, UserInterface};
use iced_wgpu::window::Compositor as WgpuCompositor;

use std::{marker::Unpin, pin::Pin, sync::Arc};

pub use async_trait::async_trait;
pub use futures::prelude::*;

use crate::{event_loop::*, surfaces::*};

pub type Element<'a, Message> = iced_native::Element<'a, Message, iced_wgpu::Renderer>;

#[async_trait(?Send)]
pub trait IcedSurface {
    type Message: std::fmt::Debug + Send;
    type ExternalEvent: std::fmt::Debug + Send;

    fn view(&mut self) -> Element<'_, Self::Message>;
    fn input_region(&self, width: i32, height: i32) -> Option<Vec<Rectangle<i32>>>;

    async fn update(&mut self, message: Self::Message);
    async fn react(&mut self, event: Self::ExternalEvent);
    // TODO: ExternalEvent | IcedEvent | LshEvent

    async fn on_pointer_enter(&mut self) {}
    async fn on_pointer_leave(&mut self) {}
    async fn on_touch_enter(&mut self) {}
    async fn on_touch_leave(&mut self) {}
}

pub struct IcedInstance<T> {
    parent: DesktopInstance,
    surface: T,

    // wayland state
    ptr_active: bool,
    scale: i32,
    leave_timeout: Option<future::Fuse<Pin<Box<dyn Future<Output = ()> + Send + 'static>>>>,
    prev_input_region: Option<Vec<Rectangle<i32>>>,
    touch_point: Option<i32>,
    touch_leave: bool,

    // iced render state
    cache: Cache,
    size: Size,
    cursor_position: Point,
    compositor: WgpuCompositor,
    renderer: <WgpuCompositor as Compositor>::Renderer,
    gpu_surface: <WgpuCompositor as Compositor>::Surface,
    swap_chain: Option<<WgpuCompositor as Compositor>::SwapChain>,
    prev_prim: iced_graphics::Primitive,
    queue: Vec<iced_native::Event>,
}

impl<T: DesktopSurface + IcedSurface> IcedInstance<T> {
    pub async fn new(
        surface: T,
        env: Environment<Env>,
        display: Display,
        queue: &EventQueue,
    ) -> IcedInstance<T> {
        let parent = DesktopInstance::new(&surface, env, display, queue);

        let mut compositor = WgpuCompositor::request(iced_wgpu::Settings {
            ..iced_wgpu::Settings::default()
        })
        .await
        .unwrap();
        let renderer = iced_wgpu::Renderer::new(compositor.create_backend());
        let gpu_surface = compositor.create_surface(&parent.raw_handle());
        parent.wl_surface.commit();
        parent.flush();

        IcedInstance {
            parent,
            surface,
            ptr_active: false,
            scale: 1,
            leave_timeout: None,
            prev_input_region: None,
            touch_point: None,
            touch_leave: false,
            cache: Cache::new(),
            size: Size::new(0.0, 0.0),
            cursor_position: Point::default(),
            compositor,
            renderer,
            gpu_surface,
            swap_chain: None,
            prev_prim: iced_graphics::Primitive::None,
            queue: Vec::new(),
        }
    }

    fn update_input_region(&mut self) {
        let reg = self
            .surface
            .input_region(self.size.width as i32, self.size.height as i32);
        if reg != self.prev_input_region {
            if let Some(ref rects) = reg {
                let wlreg = self.parent.create_region();
                for rect in rects.iter() {
                    wlreg.add(rect.x, rect.y, rect.width, rect.height);
                }
                self.parent.set_input_region(wlreg);
            } else {
                self.parent.clear_input_region();
            }
            self.parent.wl_surface.commit();
        }
        self.prev_input_region = reg;
    }

    async fn render(&mut self) {
        let swap_chain = self.swap_chain.as_mut().unwrap();

        let mut user_interface = UserInterface::build(
            self.surface.view(),
            self.size,
            self.cache.clone(),
            &mut self.renderer,
        );
        let messages = user_interface.update(
            &self.queue.drain(..).collect::<Vec<_>>(),
            self.cursor_position,
            None,
            &mut self.renderer,
        );
        let viewport = iced_graphics::Viewport::with_physical_size(
            iced_graphics::Size::new(
                self.size.width as u32 * self.scale as u32,
                self.size.height as u32 * self.scale as u32,
            ),
            self.scale as _,
        );

        if messages.is_empty() {
            let (primitive, mi) = user_interface.draw(&mut self.renderer, self.cursor_position);
            let dmg = self.prev_prim.damage(&primitive);
            self.prev_prim = primitive.clone();
            if dmg == None || dmg.map(|x| x.len()).unwrap_or(0) == 0 {
                self.cache = user_interface.into_cache();
                self.update_input_region();
                return;
            }
            let _new_mouse_cursor = self.compositor.draw::<String>(
                &mut self.renderer,
                swap_chain,
                &viewport,
                iced_core::Color::TRANSPARENT,
                &(primitive, mi),
                &[],
            );
            self.cache = user_interface.into_cache();
        } else {
            // iced-winit says we are forced to rebuild twice
            let temp_cache = user_interface.into_cache();

            for message in messages {
                self.surface.update(message).await;
            }
            self.parent.flush();

            let mut user_interface = UserInterface::build(
                self.surface.view(),
                self.size,
                temp_cache,
                &mut self.renderer,
            );
            let (primitive, mi) = user_interface.draw(&mut self.renderer, self.cursor_position);
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
                iced_core::Color::TRANSPARENT,
                &(primitive, mi),
                &[],
            );
            self.cache = user_interface.into_cache();
        }
        self.update_input_region();
    }

    fn create_swap_chain(&mut self) {
        self.swap_chain = Some(self.compositor.create_swap_chain(
            &self.gpu_surface,
            self.size.width as u32 * self.scale as u32,
            self.size.height as u32 * self.scale as u32,
        ));
        self.parent.wl_surface.set_buffer_scale(self.scale);
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
                self.parent.layer_surface.ack_configure(*serial);

                self.scale = get_surface_scale_factor(&self.parent.wl_surface);
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
                if self.parent.wl_surface.detach() == *surface {
                    self.ptr_active = true;
                    self.leave_timeout = None;
                    self.surface.on_pointer_enter().await;
                }
            }
            wl_pointer::Event::Leave { surface, .. } => {
                if self.parent.wl_surface.detach() == *surface {
                    self.ptr_active = false;
                    self.leave_timeout = Some(glib::timeout_future(200).fuse());
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
                    self.cursor_position = Point::new(*surface_x as _, *surface_y as _);
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

    async fn on_touch_event(&mut self, event: Arc<wl_touch::Event>) {
        match &*event {
            wl_touch::Event::Down { surface, id, x, y, .. } => {
                if self.parent.wl_surface.detach() != *surface {
                    return;
                }
                if self.touch_point.is_some() {
                    return;
                }
                self.touch_point = Some(*id);
                self.ptr_active = true;
                self.leave_timeout = None;
                self.cursor_position = Point::new(*x as _, *y as _);
                self.queue
                    .push(iced_native::Event::Mouse(mouse::Event::CursorMoved {
                        x: *x as _,
                        y: *y as _,
                    }));
                self.surface.on_touch_enter().await;
            },
            wl_touch::Event::Motion { id, x, y, .. } => {
                if self.touch_point != Some(*id) {
                    return;
                }
                self.cursor_position = Point::new(*x as _, *y as _);
                self.queue
                    .push(iced_native::Event::Mouse(mouse::Event::CursorMoved {
                        x: *x as _,
                        y: *y as _,
                    }));
            },
            wl_touch::Event::Up { id, .. } => {
                if self.touch_point != Some(*id) {
                    return;
                }
                self.touch_point = None;
                self.queue.push(iced_native::Event::Mouse(
                    mouse::Event::ButtonPressed(mouse::Button::Left)
                ));
                self.queue.push(iced_native::Event::Mouse(
                    mouse::Event::ButtonReleased(mouse::Button::Left)
                ));
                self.touch_leave = true;
            }
            wl_touch::Event::Frame { .. } => {
                self.render().await;
                if self.touch_leave {
                    self.surface.on_touch_leave().await;
                    self.touch_leave = false;
                    self.render().await;
                }
            }
            e => eprintln!("{:?}", e),
        }
    }

    pub async fn run(&mut self, ext_evt_src: &mut (impl Stream<Item = T::ExternalEvent> + Unpin)) {
        let seat = &self.parent.env.get_all_seats()[0];
        let mut ptr_events = wayland_event_chan(&seat.get_pointer());
        let mut touch_events = wayland_event_chan(&seat.get_touch());
        let mut layer_events = wayland_event_chan(&self.parent.layer_surface);

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
                ev = touch_events.next() => if let Some(event) = ev { self.on_touch_event(event).await },
                sc = self.parent.scale_rx.next() => if let Some(scale) = sc { self.on_scale(scale).await },
                ev = ext_evt_src.next().fuse() => if let Some(event) = ev {
                    self.surface.react(event).await;
                    self.parent.flush();
                    self.render().await
                },
                () = leave_timeout => {
                    self.surface.on_pointer_leave().await;
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
