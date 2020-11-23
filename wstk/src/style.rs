use crate::*;
pub use iced_core::{Background, Color};
use iced_graphics::*;

pub const DARK_COLOR: Color = Color::from_rgba(0.0784, 0.0784, 0.0784, 0.85);
pub const RUNNING_DARK_COLOR: Color = Color::from_rgba(0.1584, 0.1584, 0.1784, 0.85);
pub const BRIGHT_COLOR: Color = Color::from_rgba(0.874, 0.874, 0.874, 0.85);
pub const VERY_BRIGHT_COLOR: Color = Color::from_rgba(0.89, 0.89, 0.89, 0.98);
pub const SEL_COLOR: Color = Color::from_rgba(0.8, 0.8, 0.99, 0.69);

pub struct DarkBar;

impl container::StyleSheet for DarkBar {
    fn style(&self) -> container::Style {
        container::Style {
            background: Some(Background::Color(Color::from_rgba8(0, 0, 0, 0.95))),
            ..container::Style::default()
        }
    }
}

pub struct Dock(pub Color);

impl container::StyleSheet for Dock {
    fn style(&self) -> container::Style {
        container::Style {
            background: Some(Background::Color(self.0)),
            text_color: Some(Color::WHITE),
            border_radius: 3.0,
            ..container::Style::default()
        }
    }
}

impl button::StyleSheet for Dock {
    fn active(&self) -> button::Style {
        button::Style {
            background: None,
            ..button::Style::default()
        }
    }
}

pub struct Toplevel;

impl button::StyleSheet for Toplevel {
    fn active(&self) -> button::Style {
        button::Style {
            background: Some(Background::Color(DARK_COLOR)),
            border_radius: 3.0,
            text_color: Color::WHITE,
            ..button::Style::default()
        }
    }

    fn hovered(&self) -> button::Style {
        button::Style {
            background: Some(Background::Color(Color::from_rgba8(69, 69, 69, 0.85))),
            ..self.active()
        }
    }

    fn pressed(&self) -> button::Style {
        button::Style {
            border_width: 1.0,
            border_color: Color::WHITE,
            ..self.hovered()
        }
    }
}

pub struct Dialog;

impl container::StyleSheet for Dialog {
    fn style(&self) -> container::Style {
        container::Style {
            background: Some(Background::Color(DARK_COLOR)),
            border_width: 1.0,
            border_color: BRIGHT_COLOR,
            border_radius: 3.0,
            text_color: Some(VERY_BRIGHT_COLOR),
            ..container::Style::default()
        }
    }
}

impl text_input::StyleSheet for Dialog {
    fn active(&self) -> text_input::Style {
        text_input::Style {
            background: Background::Color(Color::from_rgba8(100, 100, 100, 0.45)),
            border_radius: 3.0,
            border_width: 1.0,
            border_color: Color::from_rgba8(255, 255, 255, 0.45),
        }
    }

    fn focused(&self) -> text_input::Style {
        text_input::Style {
            background: Background::Color(VERY_BRIGHT_COLOR),
            border_color: BRIGHT_COLOR,
            ..self.active()
        }
    }

    fn hovered(&self) -> text_input::Style {
        text_input::Style {
            background: Background::Color(Color::from_rgba8(200, 200, 200, 0.55)),
            border_color: Color::from_rgba8(255, 255, 255, 0.55),
            ..self.active()
        }
    }

    fn placeholder_color(&self) -> Color {
        Color::from_rgb(0.4, 0.4, 0.4)
    }

    fn value_color(&self) -> Color {
        DARK_COLOR
    }

    fn selection_color(&self) -> Color {
        SEL_COLOR
    }
}

pub enum ActionType {
    Bad,
    Good,
}

pub struct Action(pub ActionType);

impl button::StyleSheet for Action {
    fn active(&self) -> button::Style {
        button::Style {
            background: Some(Background::Color(match &self.0 {
                ActionType::Bad => Color::from_rgba8(155, 55, 55, 0.85),
                ActionType::Good => Color::from_rgba8(55, 155, 55, 0.85),
            })),
            border_radius: 69.0,
            border_width: 1.0,
            border_color: match &self.0 {
                ActionType::Bad => Color::from_rgba8(205, 105, 105, 0.85),
                ActionType::Good => Color::from_rgba8(105, 205, 105, 0.85),
            },
            text_color: BRIGHT_COLOR,
            ..button::Style::default()
        }
    }

    fn hovered(&self) -> button::Style {
        button::Style {
            background: Some(Background::Color(match &self.0 {
                ActionType::Bad => Color::from_rgba8(255, 155, 155, 0.85),
                ActionType::Good => Color::from_rgba8(155, 255, 155, 0.85),
            })),
            border_width: 0.0,
            text_color: DARK_COLOR,
            ..self.active()
        }
    }

    fn pressed(&self) -> button::Style {
        button::Style {
            border_color: BRIGHT_COLOR,
            border_width: 1.0,
            ..self.hovered()
        }
    }
}
