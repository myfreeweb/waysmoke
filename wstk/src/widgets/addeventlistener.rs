//! The antidote to iced's annoying rigidity and inflexibility,
//! the equivalent of anything.addEventListener('mouseover', ..) :P

use iced_native::*;
use std::hash::Hash;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct State {
    is_hovered: bool,
}

pub struct AddEventListener<'a, Message, Renderer: self::Renderer> {
    state: &'a mut State,
    content: Element<'a, Message, Renderer>,
    pointer_enter: Option<Message>,
    pointer_leave: Option<Message>,
}

impl<'a, Message, Renderer> AddEventListener<'a, Message, Renderer>
where
    Renderer: self::Renderer,
{
    pub fn new<T>(state: &'a mut State, content: T) -> Self
    where
        T: Into<Element<'a, Message, Renderer>>,
    {
        AddEventListener {
            state,
            content: content.into(),
            pointer_enter: None,
            pointer_leave: None,
        }
    }

    pub fn on_pointer_enter(mut self, msg: Message) -> Self {
        self.pointer_enter = Some(msg);
        self
    }

    pub fn on_pointer_leave(mut self, msg: Message) -> Self {
        self.pointer_leave = Some(msg);
        self
    }
}

impl<'a, Message, Renderer> Widget<Message, Renderer> for AddEventListener<'a, Message, Renderer>
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
        let content = self.content.layout(renderer, &limits.loose());
        let size = limits.resolve(content.size());
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
        let bounds = layout.bounds();
        let is_mouse_over = bounds.contains(cursor_position);
        if is_mouse_over && !self.state.is_hovered {
            if let Some(ref msg) = self.pointer_enter {
                messages.push(msg.clone());
            }
        }
        if !is_mouse_over && self.state.is_hovered {
            if let Some(ref msg) = self.pointer_leave {
                messages.push(msg.clone());
            }
        }
        self.state.is_hovered = is_mouse_over;

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
    ) -> Renderer::Output {
        renderer.draw(
            defaults,
            cursor_position,
            &self.content,
            layout.children().next().unwrap(),
        )
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
        content: &Element<'_, Message, Self>,
        content_layout: Layout<'_>,
    ) -> Self::Output;
}

impl<'a, Message, Renderer> From<AddEventListener<'a, Message, Renderer>>
    for Element<'a, Message, Renderer>
where
    Renderer: 'a + self::Renderer,
    Message: 'a + Clone,
{
    fn from(x: AddEventListener<'a, Message, Renderer>) -> Element<'a, Message, Renderer> {
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
        content: &Element<'_, Message, Self>,
        content_layout: Layout<'_>,
    ) -> Self::Output {
        content.draw(self, defaults, content_layout, cursor_position)
    }
}
