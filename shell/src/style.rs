use iced_core::{Background, Color};
use iced_graphics::*;
use wstk::*;

pub const DARK_COLOR: Color = Color::from_rgba(0.0784, 0.0784, 0.0784, 0.85);

pub struct DarkBar;

impl container::StyleSheet for DarkBar {
    fn style(&self) -> container::Style {
        container::Style {
            background: Some(Background::Color(Color::from_rgba8(0, 0, 0, 0.95))),
            ..container::Style::default()
        }
    }
}

pub struct Dock;

impl container::StyleSheet for Dock {
    fn style(&self) -> container::Style {
        container::Style {
            background: Some(Background::Color(DARK_COLOR)),
            text_color: Some(Color::WHITE),
            border_radius: 3,
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
            border_radius: 3,
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
            border_width: 1,
            border_color: Color::WHITE,
            ..self.hovered()
        }
    }
}
