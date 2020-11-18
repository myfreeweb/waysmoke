//! A way to track layout regions of particular areas

use iced_native::*;
use std::{cell::Cell, hash::Hash};

pub struct GetRegion<'a, Message, Renderer: self::Renderer> {
    // The Cell helps both with the immutable &self in layout(), and with multi-borrows in the consumer
    state: &'a Cell<Rectangle>,
    content: Element<'a, Message, Renderer>,
    horizontal_alignment: Align,
    vertical_alignment: Align,
}

impl<'a, Message, Renderer> GetRegion<'a, Message, Renderer>
where
    Renderer: self::Renderer,
{
    pub fn new<T>(state: &'a Cell<Rectangle>, content: T) -> Self
    where
        T: Into<Element<'a, Message, Renderer>>,
    {
        GetRegion {
            state,
            content: content.into(),
            horizontal_alignment: Align::Start,
            vertical_alignment: Align::Start,
        }
    }

    pub fn center_x(mut self) -> Self {
        self.horizontal_alignment = Align::Center;
        self
    }

    pub fn center_y(mut self) -> Self {
        self.vertical_alignment = Align::Center;
        self
    }
}

impl<'a, Message, Renderer> Widget<Message, Renderer> for GetRegion<'a, Message, Renderer>
where
    Renderer: self::Renderer,
    Message: Clone,
{
    fn width(&self) -> Length {
        Length::Shrink
    }

    fn height(&self) -> Length {
        Length::Shrink
    }

    fn layout(&self, renderer: &Renderer, limits: &layout::Limits) -> layout::Node {
        let limits = limits.width(Length::Shrink).height(Length::Shrink);
        let mut content = self.content.layout(renderer, &limits.loose());
        let size = limits.resolve(content.size());
        content.align(self.horizontal_alignment, self.vertical_alignment, size);
        layout::Node::with_children(size, vec![content])
    }

    fn on_event(
        &mut self,
        event: Event,
        layout: Layout<'_>,
        cursor_position: Point,
        messages: &mut Vec<Message>,
        renderer: &Renderer,
        clipboard: Option<&dyn Clipboard>,
    ) {
        self.content.on_event(
            event,
            layout.children().next().unwrap(),
            cursor_position,
            messages,
            renderer,
            clipboard,
        )
    }

    fn draw(
        &self,
        renderer: &mut Renderer,
        defaults: &Renderer::Defaults,
        layout: Layout<'_>,
        cursor_position: Point,
        viewport: &Rectangle,
    ) -> Renderer::Output {
        let child_layout = layout.children().next().unwrap();
        self.state.set(child_layout.bounds());
        renderer.draw(defaults, cursor_position, viewport, &self.content, child_layout)
    }

    fn hash_layout(&self, state: &mut Hasher) {
        struct Marker;
        std::any::TypeId::of::<Marker>().hash(state);

        self.content.hash_layout(state);
    }
}

pub trait Renderer: iced_native::Renderer {
    fn draw<Message>(
        &mut self,
        defaults: &Self::Defaults,
        cursor_position: Point,
        viewport: &Rectangle,
        content: &Element<'_, Message, Self>,
        content_layout: Layout<'_>,
    ) -> Self::Output;
}

impl<'a, Message, Renderer> From<GetRegion<'a, Message, Renderer>> for Element<'a, Message, Renderer>
where
    Renderer: 'a + self::Renderer,
    Message: 'a + Clone,
{
    fn from(x: GetRegion<'a, Message, Renderer>) -> Element<'a, Message, Renderer> {
        Element::new(x)
    }
}

impl<B> Renderer for iced_graphics::Renderer<B>
where
    B: iced_graphics::Backend,
{
    fn draw<Message>(
        &mut self,
        defaults: &iced_graphics::Defaults,
        cursor_position: Point,
        viewport: &Rectangle,
        content: &Element<'_, Message, Self>,
        content_layout: Layout<'_>,
    ) -> Self::Output {
        content.draw(self, defaults, content_layout, cursor_position, viewport)
    }
}
