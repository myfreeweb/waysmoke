#![recursion_limit = "1024"]

#[macro_use]
pub mod event_loop;
pub use event_loop::*;

#[macro_use]
pub mod toplevels;
pub use toplevels::*;

pub mod surfaces;
pub use surfaces::*;

pub mod iced;
pub use iced::*;

pub mod widgets;
pub use widgets::*;

pub mod handle;

pub use iced_core;
pub use iced_graphics;
pub use iced_native;

pub use bus_queue::flavors::arc_swap as bus;

#[macro_export]
macro_rules! wstk_main {
    ( $fun:ident ) => {
        static mut LOL: Option<EventQueue> = None;

        fn main() -> Result<(), Box<dyn std::error::Error>> {
            let main = glib::MainLoop::new(None, false);
            glib::MainContext::default().acquire();
            let (env, disp, queue) = make_env()?;
            let queue = unsafe {
                LOL = Some(queue);
                glib_add_wayland(LOL.as_mut().unwrap());
                LOL.as_ref().unwrap()
            };
            glib::MainContext::default().spawn_local($fun(env, disp, queue));
            main.run();
            Ok(())
        }
    };
}
