use wstk::*;

#[derive(Debug, Clone)]
enum TestMsg {
    IncrementPressed,
    DecrementPressed,
}

#[derive(Debug, Clone)]
enum TestExtEvt {
    Sig,
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
            layer_surface::Anchor::Left
                | layer_surface::Anchor::Right
                | layer_surface::Anchor::Bottom,
        );
        layer_surface.set_size(0, 34);
        // layer_surface.set_exclusive_zone(24);
    }
}

use iced_graphics::*;
pub struct Kontainer;

impl container::StyleSheet for Kontainer {
    fn style(&self) -> container::Style {
        container::Style {
            background: Some(iced_core::Background::Color(iced_core::Color::from_rgba8(
                0x36, 0x39, 0x3F, 0.3,
            ))),
            text_color: Some(iced_core::Color::WHITE),
            ..container::Style::default()
        }
    }
}

#[async_trait]
impl IcedWidget for Test {
    type Message = TestMsg;
    type ExternalEvent = TestExtEvt;

    fn view(&mut self) -> Element<TestMsg> {
        use iced_native::*;

        let row = Row::new()
            .align_items(Align::Center)
            .padding(2)
            .push(
                Button::new(&mut self.increment_button, Text::new("Incr"))
                    .on_press(TestMsg::IncrementPressed),
            )
            .push(Text::new(self.value.to_string()).size(20))
            .push(
                Button::new(&mut self.decrement_button, Text::new("Decr"))
                    .on_press(TestMsg::DecrementPressed),
            );

        Container::new(row)
            .style(Kontainer)
            // .width(Length::Fill)
            // .height(Length::Fill)
            .center_x()
            .center_y()
            .into()
    }

    async fn update(&mut self, message: TestMsg) {
        match message {
            TestMsg::IncrementPressed => {
                self.value += 1;
            }
            TestMsg::DecrementPressed => {
                self.value -= 1;
            }
        }
    }

    async fn react(&mut self, _event: TestExtEvt) {
        self.value += 10;
    }

    async fn on_rendered(&mut self, ls: layer_surface::ZwlrLayerSurfaceV1) {
        ls.set_exclusive_zone(self.value);
    }
}

wstk_main! {
async fn main(env: Environment<Env>, display: WlDisplay, queue: &EventQueue) {
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

    let mut test = IcedInstance::new(Test::default(), env.clone(), display.clone(), queue).await;
    // let mut test2 = IcedInstance::new(Test::default(), env.clone(), display.clone(), queue).await;
    let mut pend = glib::unix_signal_stream(30).map(|()| TestExtEvt::Sig);
    futures::join!(
        test.run(&mut pend),
        // test2.run(),
    );
}
}
