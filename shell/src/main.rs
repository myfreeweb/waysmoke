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

    let (new_outputs_tx, new_outputs_rx) = bus::bounded(1);
    let new_outputs_tx = std::rc::Rc::new(std::cell::RefCell::new(new_outputs_tx));
    let _listner_handle = env.listen_for_outputs(move |output, info, _| {
        if info.obsolete {
            return;
        }
        let tx = new_outputs_tx.clone();
        glib::MainContext::default()
            .spawn_local(async move { tx.borrow_mut().send(output).await.unwrap() });
    });

    let toplevel_updates = env.with_inner(|i| i.toplevel_updates());

    let power = svc::power::PowerService::new(&dbus).await;
    let media = svc::media::MediaService::new(&dbus).await;

    let seat = env.get_all_seats()[0].detach();

    let dctx = dock::DockCtx {
        seat,
        toplevel_updates,
        power,
        media,
    };

    let mut mm = MultiMonitor::new(
        Box::new(|output| {
            IcedInstance::new(
                dock::Dock::new(dctx.clone()),
                env.clone(),
                display.clone(),
                output,
            )
        }),
        &env,
        new_outputs_rx.clone(),
    )
    .await;

    while mm.run().await {}
}

wstk_main!(main_);
