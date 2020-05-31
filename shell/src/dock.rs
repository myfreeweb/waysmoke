use crate::{style, util::*};
use wstk::*;

pub enum LayerShellAction {
    SetSize(u32),
}

#[derive(Debug, Clone)]
pub enum Msg {
    IncrementPressed,
    DecrementPressed,
}

#[derive(Debug, Clone)]
pub enum Evt {
    Sig,
}

#[derive(Default)]
pub struct Dock {
    shown: bool,
    ic1: Option<icons::IconHandle>,
    ic2: Option<icons::IconHandle>,
    value: i32,
    increment_button: iced_native::button::State,
    decrement_button: iced_native::button::State,
    next_layer_shell_action: Option<LayerShellAction>,
}

impl DesktopWidget for Dock {
    fn setup_lsh(&self, layer_surface: &Main<layer_surface::ZwlrLayerSurfaceV1>) {
        layer_surface.set_anchor(
            layer_surface::Anchor::Left
                | layer_surface::Anchor::Right
                | layer_surface::Anchor::Bottom,
        );
        layer_surface.set_size(0, 10);
        layer_surface.set_exclusive_zone(10);
    }
}

#[async_trait]
impl IcedWidget for Dock {
    type Message = Msg;
    type ExternalEvent = Evt;

    fn view(&mut self) -> Element<Self::Message> {
        use iced_native::*;

        let mut col = Column::new().width(Length::Fill);

        if self.shown {
            if self.ic1.is_none() {
                self.ic1 = Some(icons::IconHandle::from_path(
                    apps::App::lookup("org.gnome.Weather", None).icon(),
                ));
            }
            if self.ic2.is_none() {
                self.ic2 = Some(icons::IconHandle::from_path(
                    apps::App::lookup("gtk3-demo", None).icon(),
                ));
            }
            let row = Row::new()
                .align_items(Align::Center)
                .spacing(20)
                .push(
                    Button::new(
                        &mut self.increment_button,
                        self.ic1.as_ref().unwrap().clone().widget(),
                    )
                    .style(style::Dock)
                    .on_press(Msg::IncrementPressed),
                )
                .push(Text::new(self.value.to_string()).size(20))
                .push(
                    Button::new(
                        &mut self.decrement_button,
                        self.ic2.as_ref().unwrap().clone().widget(),
                    )
                    .style(style::Dock)
                    .on_press(Msg::DecrementPressed),
                );

            let dock = Container::new(
                Container::new(row)
                    .style(style::Dock)
                    .width(Length::Shrink)
                    .height(Length::Units(69))
                    .center_x()
                    .center_y()
                    .padding(4),
            )
            .width(Length::Fill)
            .height(Length::Units(75))
            .center_x();

            col = col.push(dock);
        }

        let bar = Container::new(
            Container::new(Text::new("".to_string()).size(0))
                .style(style::WhiteStripe)
                .width(Length::Units(128))
                .height(Length::Units(4)),
        )
        .style(style::DarkBar)
        .width(Length::Fill)
        .height(Length::Units(10))
        .center_x()
        .center_y();

        col.push(bar).into()
    }

    async fn update(&mut self, message: Self::Message) {
        match message {
            Msg::IncrementPressed => {
                self.value += 1;
            }
            Msg::DecrementPressed => {
                self.value -= 1;
            }
        }
    }

    async fn react(&mut self, _event: Self::ExternalEvent) {
        self.value += 10;
    }

    async fn on_rendered(&mut self, layer_surface: layer_surface::ZwlrLayerSurfaceV1) {
        if let Some(act) = self.next_layer_shell_action.take() {
            match act {
                LayerShellAction::SetSize(sz) => layer_surface.set_size(0, sz),
            }
        }
    }

    async fn on_pointer_enter(&mut self) {
        self.shown = true;
        self.next_layer_shell_action = Some(LayerShellAction::SetSize(85));
    }

    async fn on_pointer_leave(&mut self) {
        self.shown = false;
        self.next_layer_shell_action = Some(LayerShellAction::SetSize(10));
    }
}
