use wstk::*;

#[derive(Clone)]
pub enum IconHandle {
    Vector(iced_native::svg::Handle),
    Raster(iced_native::image::Handle),
}

impl IconHandle {
    pub fn from_path(path: linicon::IconPath) -> IconHandle {
        match path.icon_type {
            linicon::IconType::SVG => {
                IconHandle::Vector(iced_native::svg::Handle::from_path(path.path))
            }
            _ => IconHandle::Raster(iced_native::image::Handle::from_path(path.path)),
        }
    }

    pub fn widget<'a, Message>(self) -> Element<'a, Message> {
        use iced_native::{Image, Length, Svg};
        match self {
            IconHandle::Raster(h) => Image::new(h)
                .width(Length::Units(64))
                .height(Length::Units(64))
                .into(),
            IconHandle::Vector(h) => Svg::new(h)
                .width(Length::Units(64))
                .height(Length::Units(64))
                .into(),
        }
    }
}
