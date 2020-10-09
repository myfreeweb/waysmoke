use futures::stream::select as add_stream;
use iced_native::*;
use wstk::*;

mod dock;
mod style;
mod svc;
mod util;
mod wallpaper;

async fn main_(env: Environment<Env>, display: Display, queue: &EventQueue) {
    // TODO: multi-monitor handling
    // let output_handler = move |output: wl_output::WlOutput, info: &OutputInfo| {
    //     eprintln!("Output {:?}", info);
    // };
    // let _listner_handle =
    //     env.listen_for_outputs(move |output, info, _| output_handler(output, info));
    // display.flush().unwrap();
    // for output in env.get_all_outputs() {
    //     if let Some(info) = with_output_info(&output, Clone::clone) {
    //         println!("Output {:?}", info);
    //     }
    // }

    // let app = gio::Application::new(
    //     Some("technology.unrelenting.waysmoke.Shell"),
    //     gio::ApplicationFlags::default(),
    // );
    // app.register::<gio::Cancellable>(None).unwrap();
    // let dbus = app.get_dbus_connection().unwrap();

    let (toplevels, toplevel_updates) = env.with_inner(|i| (i.toplevels(), i.toplevel_updates()));

    let (power, power_updates) = svc::power::PowerService::new().await;

    let mut dock_evts = add_stream(
        toplevel_updates.map(|()| dock::Evt::ToplevelsChanged),
        power_updates.map(|ps| dock::Evt::PowerChanged(ps)),
    );

    let seat = env.get_all_seats()[0].detach();
    let mut dock = IcedInstance::new(
        dock::Dock::new(dock::DockCtx {
            seat,
            toplevels,
            power,
        }),
        env.clone(),
        display.clone(),
        queue,
    )
    .await;

    let mut wallpaper = wallpaper::Wallpaper::new(env.clone(), display.clone(), queue).await;

    futures::join!(dock.run(&mut dock_evts), wallpaper.run());
}

wstk_main!(main_);
