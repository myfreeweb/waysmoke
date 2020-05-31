#![recursion_limit = "1024"]

#[macro_use]
pub mod event_loop;
pub use event_loop::*;

pub mod iced;
pub use iced::*;

pub mod handle;

pub use iced_core;
pub use iced_graphics;
pub use iced_native;

#[macro_export]
macro_rules! wstk_main {
    ( async fn main ( $env:ident : Environment<Env>, $disp:ident : WlDisplay, $queue:ident : &EventQueue ) { $($body:tt)* } ) => {
        static mut LOL: Option<EventQueue> = None;

        fn main() -> Result<(), Box<dyn std::error::Error>> {
            let ($env, $disp, queue) = make_env()?;
            let main = glib::MainLoop::new(None, false);
            glib::MainContext::default().acquire();
            let $queue = unsafe {
                LOL = Some(queue);
                glib_add_wayland(LOL.as_mut().unwrap());
                LOL.as_ref().unwrap()
            };
            glib::MainContext::default().spawn_local(async move { $($body)* });
            main.run();
            Ok(())
        }
    }
}
