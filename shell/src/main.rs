use gio::ApplicationExt;
use wstk::*;

mod dock;
mod style;
mod svc;
mod util;

async fn main_(env: &Environment<Env>, display: &Display) {
    let app = gio::Application::new(
        Some("technology.unrelenting.waysmoke.Shell"),
        gio::ApplicationFlags::default(),
    );
    app.register::<gio::Cancellable>(None).unwrap();
    let dbus = app.get_dbus_connection().unwrap();

    let services: &'static _ = Box::leak(Box::new(svc::Services {
        seat: env.get_all_seats()[0].detach(),
        toplevels: env.with_inner(|i| i.toplevel_service()),
        power: svc::power::PowerService::new(&dbus).await,
        media: svc::media::MediaService::new(&dbus).await,
    }));

    let mut dock_mm = MultiMonitor::new(
        Box::new(|output, _output_info| {
            IcedInstance::new(dock::Dock::new(services), env.clone(), display.clone(), output)
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
