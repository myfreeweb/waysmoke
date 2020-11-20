#![recursion_limit = "1024"]

#[macro_use]
pub mod event_loop;
pub use event_loop::*;

#[macro_use]
pub mod toplevels;
pub use toplevels::*;

pub mod run;
pub use run::*;

pub mod multimonitor;
pub use multimonitor::*;

pub mod surfaces;
pub use surfaces::*;

pub mod iced;
pub use iced::*;

pub mod widgets;
pub use widgets::*;

pub mod style;

pub mod handle;

pub use iced_core;
pub use iced_graphics;
pub use iced_native;

pub use event_listener;

#[macro_export]
macro_rules! wstk_main {
    ( $fun:ident ) => {
        fn main() -> Result<(), Box<dyn std::error::Error>> {
            let main = glib::MainLoop::new(None, false);
            glib::MainContext::default().acquire();
            let (env, disp, queue) = make_env()?;
            let env: &'static Environment<Env> = Box::leak(Box::new(env));
            let disp: &'static Display = Box::leak(Box::new(disp));
            let queue: &'static mut EventQueue = Box::leak(Box::new(queue));
            glib_add_wayland(queue);
            glib::MainContext::default().spawn_local($fun(env, disp));
            main.run();
            Ok(())
        }
    };
}
