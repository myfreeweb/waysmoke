use crate::{dock::*, style};
use std::sync::Arc;

#[derive(Debug, Clone)]
pub enum Msg {
    ActivateApp,
    ActivateToplevel(usize),
}

pub struct AppDocklet {
    pub app: apps::App,
    pub icon: wstk::ImageHandle,
    pub button: iced_native::button::State,
    pub evl: addeventlistener::State,
    pub toplevels_scrollable: iced_native::scrollable::State,
    pub toplevels_buttons: Vec<iced_native::button::State>,
    pub seat: wl_seat::WlSeat,
    pub rx: wstk::bus::Subscriber<
        HashMap<wstk::toplevels::ToplevelKey, wstk::toplevels::ToplevelState>,
    >,
    pub toplevels: Arc<HashMap<wstk::toplevels::ToplevelKey, wstk::toplevels::ToplevelState>>,
}

impl AppDocklet {
    pub fn new(
        app: apps::App,
        seat: wl_seat::WlSeat,
        rx: wstk::bus::Subscriber<
            HashMap<wstk::toplevels::ToplevelKey, wstk::toplevels::ToplevelState>,
        >,
    ) -> AppDocklet {
        let icon = app
            .icon()
            .map(icons::icon_from_path)
            .unwrap_or_else(|| UNKNOWN_ICON.clone());
        AppDocklet {
            app,
            icon,
            button: Default::default(),
            evl: Default::default(),
            toplevels_scrollable: Default::default(),
            toplevels_buttons: Default::default(),
            seat,
            rx,
            toplevels: Arc::new(HashMap::new()),
        }
    }

    pub fn id(&self) -> &str {
        &self.app.id
    }

    pub fn from_id(
        id: &str,
        seat: wl_seat::WlSeat,
        rx: wstk::bus::Subscriber<
            HashMap<wstk::toplevels::ToplevelKey, wstk::toplevels::ToplevelState>,
        >,
    ) -> Option<AppDocklet> {
        apps::App::lookup(id).map(|a| AppDocklet::new(a, seat, rx))
    }
}

#[async_trait(?Send)]
impl Docklet for AppDocklet {
    fn widget(&mut self) -> Element<DockletMsg> {
        use iced_native::*;

        let running = our_toplevels(&self.toplevels, self.id()).next().is_some();

        let big_button = Button::new(&mut self.button, icons::icon_widget(self.icon.clone()))
            .style(style::Dock(style::DARK_COLOR))
            .padding(APP_PADDING)
            .on_press(DockletMsg::App(Msg::ActivateApp));

        let listener =
            AddEventListener::new(&mut self.evl, big_button).on_pointer_enter(DockletMsg::Hover);

        Container::new(listener)
            .center_x()
            .center_y()
            .style(style::Dock(if running {
                style::RUNNING_DARK_COLOR
            } else {
                style::DARK_COLOR
            }))
            .into()
    }

    fn width(&self) -> u16 {
        // TODO: will be dynamic based on extras
        icons::ICON_SIZE + APP_PADDING * 2
    }

    fn retained_icon(&self) -> Option<wstk::ImageHandle> {
        Some(self.icon.clone())
    }

    fn popover(&mut self) -> Option<Element<DockletMsg>> {
        use iced_native::*;

        let appid = &self.app.id;
        while self.toplevels_buttons.len() < our_toplevels(&self.toplevels, &self.app.id).count() {
            self.toplevels_buttons.push(Default::default());
        }
        let mut btns = Scrollable::new(&mut self.toplevels_scrollable).spacing(2);
        // ugh, fold results in closure lifetime issues
        for (i, topl) in our_toplevels(&self.toplevels, &self.app.id).enumerate() {
            btns = btns.push(
                Button::new(
                    // and even here it complains about "multiple" borrows of self.toplevels_buttons >_<
                    unsafe { &mut *(&mut self.toplevels_buttons[i] as *mut _) },
                    Text::new(topl.title.clone()).size(14),
                )
                .style(style::Toplevel)
                .width(Length::Fill)
                .on_press(DockletMsg::App(Msg::ActivateToplevel(i))),
            )
        }
        let title = Text::new(
            self.app
                .info
                .get_name()
                .map(|x| x.to_owned())
                .unwrap_or("<untitled>".to_owned()),
        )
        .width(Length::Fill)
        .horizontal_alignment(HorizontalAlignment::Center)
        .size(16);
        Some(
            Column::new()
                .width(Length::Units(TOPLEVELS_WIDTH))
                .push(title)
                .push(btns)
                .spacing(DOCK_PADDING)
                .into(),
        )
    }

    fn update(&mut self, msg: DockletMsg) {
        match msg {
            DockletMsg::App(Msg::ActivateApp) => {
                for topl in our_toplevels(&self.toplevels, &self.app.id) {
                    topl.handle.activate(&self.seat);
                    return;
                }
                self.app
                    .info
                    .launch::<gio::AppLaunchContext>(&[], None)
                    .unwrap()
            }
            DockletMsg::App(Msg::ActivateToplevel(topli)) => {
                our_toplevels(&self.toplevels, &self.app.id)
                    .nth(topli)
                    .unwrap()
                    .handle
                    .activate(&self.seat);
            }
            _ => (),
        }
    }

    async fn run(&mut self) {
        self.toplevels = self.rx.next().await.unwrap();
    }
}

// can't just have a method on self because rustc can't see through
// the function boundary to know which parts of self are actually borrowed
fn our_toplevels<'a>(
    toplevels: &'a Arc<HashMap<wstk::toplevels::ToplevelKey, wstk::toplevels::ToplevelState>>,
    id: &'a str,
) -> impl Iterator<Item = &'a wstk::toplevels::ToplevelState> {
    toplevels.values().filter(move |topl| topl.matches_id(id))
}
