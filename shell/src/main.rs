use wstk::*;

#[derive(Debug, Clone)]
enum TestMsg {
    IncrementPressed,
    DecrementPressed,
}

#[derive(Default)]
struct Test {
    value: i32,
    increment_button: iced_native::button::State,
    decrement_button: iced_native::button::State,
}

impl DesktopWidget for Test {
    fn setup_lsh(&self, layer_surface: &Main<layer_surface::ZwlrLayerSurfaceV1>) {
        layer_surface.set_anchor(
            layer_surface::Anchor::Top
                | layer_surface::Anchor::Right
                | layer_surface::Anchor::Bottom,
        );
        layer_surface.set_size(90, 0);
        layer_surface.set_exclusive_zone(90);
    }
}

impl IcedWidget for Test {
    type Message = TestMsg;

    fn view(&mut self) -> Element<TestMsg> {
        use iced_native::*;

        Column::new()
            .padding(20)
            .align_items(Align::Center)
            .push(
                Button::new(&mut self.increment_button, Text::new("Incr"))
                    .on_press(TestMsg::IncrementPressed),
            )
            .push(Text::new(self.value.to_string()).size(50))
            .push(
                Button::new(&mut self.decrement_button, Text::new("Decr"))
                    .on_press(TestMsg::DecrementPressed),
            )
            .into()
    }

    fn update(&mut self, message: TestMsg) -> iced_native::Command<TestMsg> {
        match message {
            TestMsg::IncrementPressed => {
                self.value += 1;
            }
            TestMsg::DecrementPressed => {
                self.value -= 1;
            }
        }
        iced_native::Command::none()
    }
}

wstk_main! {
async fn main(env: Environment<Env>, display: WlDisplay) {
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

    let mut test = IcedInstance::new(Test::default(), env.clone(), display.clone()).await;
    let mut test2 = IcedInstance::new(Test::default(), env.clone(), display.clone()).await;

    futures::join!(
        test.run(),
        test2.run(),
    );
}
}
