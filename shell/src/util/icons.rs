use wstk::*;

pub const ICON_SIZE: u16 = 48;

pub fn icon_from_path(path: linicon::IconPath) -> ImageHandle {
    match path.icon_type {
        linicon::IconType::SVG => {
            ImageHandle::Vector(iced_native::svg::Handle::from_path(path.path))
        }
        _ => ImageHandle::Raster(iced_native::image::Handle::from_path(path.path)),
    }
}

pub fn icon_widget<'a, Message>(icon: ImageHandle) -> Element<'a, Message> {
    use iced_native::{Image, Length, Svg};
    match icon {
        ImageHandle::Raster(h) => Image::new(h)
            .width(Length::Units(ICON_SIZE))
            .height(Length::Units(ICON_SIZE))
            .into(),
        ImageHandle::Vector(h) => Svg::new(h)
            .width(Length::Units(ICON_SIZE))
            .height(Length::Units(ICON_SIZE))
            .into(),
    }
}
