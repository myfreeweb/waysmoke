use gio::ApplicationExt;
use wstk::*;

mod dock;
mod svc;
mod util;

async fn main_(env: &Environment<Env>, display: &Display) {
    let app = gio::Application::new(
        Some("technology.unrelenting.waysmoke.Shell"),
        gio::ApplicationFlags::default(),
    );
    app.register::<gio::Cancellable>(None).unwrap();
    let session_bus = app.get_dbus_connection().unwrap();

    let services: &'static _ = Box::leak(Box::new(svc::Services {
        seat: env.get_all_seats()[0].detach(),
        toplevels: env.with_inner(|i| i.toplevel_service()),
        power: svc::power::PowerService::new(&session_bus).await,
        media: svc::media::MediaService::new(&session_bus).await,
    }));

    let mut dock_mm = MultiMonitor::new(
        Box::new(|output, _output_info| {
            IcedInstance::new(dock::Dock::new(services), env.clone(), display.clone(), output).boxed_local()
        }),
        &env,
    )
    .await;

    loop {
        futures::select! {
            _ = dock_mm.run().fuse() => (),
        }
    }
}

wstk_main!(main_);
