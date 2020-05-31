use iced_core::{Background, Color};
use iced_graphics::*;
use wstk::*;

pub struct DarkBar;

impl container::StyleSheet for DarkBar {
    fn style(&self) -> container::Style {
        container::Style {
            background: Some(Background::Color(Color::from_rgba8(0, 0, 0, 0.95))),
            ..container::Style::default()
        }
    }
}

pub struct WhiteStripe;

impl container::StyleSheet for WhiteStripe {
    fn style(&self) -> container::Style {
        container::Style {
            background: Some(Background::Color(Color::WHITE)),
            border_radius: 2,
            ..container::Style::default()
        }
    }
}

pub struct Dock;

impl container::StyleSheet for Dock {
    fn style(&self) -> container::Style {
        container::Style {
            background: Some(Background::Color(Color::from_rgba8(20, 20, 20, 0.85))),
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
