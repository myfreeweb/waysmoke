use iced_native::*;
use wstk::*;

mod dock;
mod style;
mod util;

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

    let mut test =
        IcedInstance::new(dock::Dock::default(), env.clone(), display.clone(), queue).await;
    // let mut test2 = IcedInstance::new(Test::default(), env.clone(), display.clone(), queue).await;
    let mut pend = glib::unix_signal_stream(30).map(|()| dock::Evt::Sig);
    futures::join!(
        test.run(&mut pend),
        // test2.run(),
    );
}

wstk_main!(main_);
