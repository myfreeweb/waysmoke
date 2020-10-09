use crate::{dock::*, style};

#[derive(Debug, Clone)]
pub enum Msg {
    ActivateApp,
    ActivateToplevel(usize),
}

pub struct AppDocklet {
    pub app: apps::App,
    pub icon: icons::IconHandle,
    pub button: iced_native::button::State,
    pub evl: addeventlistener::State,
    pub toplevels_scrollable: iced_native::scrollable::State,
    pub toplevels_buttons: Vec<iced_native::button::State>,
}

impl AppDocklet {
    pub fn new(app: apps::App) -> AppDocklet {
        let icon = app
            .icon()
            .map(icons::IconHandle::from_path)
            .unwrap_or_else(|| UNKNOWN_ICON.clone());
        AppDocklet {
            app,
            icon,
            button: Default::default(),
            evl: Default::default(),
            toplevels_scrollable: Default::default(),
            toplevels_buttons: Default::default(),
        }
    }

    pub fn id(&self) -> &str {
        &self.app.id
    }

    pub fn from_id(id: &str) -> Option<AppDocklet> {
        apps::App::lookup(id).map(AppDocklet::new)
    }
}

impl Docklet for AppDocklet {
    fn widget(&mut self, ctx: &DockCtx) -> Element<DockletMsg> {
        use iced_native::*;

        let toplevels = ctx.toplevels.borrow();
        let running = toplevels.values().any(|topl| topl.matches_id(self.id()));

        let big_button = Button::new(&mut self.button, self.icon.clone().widget())
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

    fn overhang(&mut self, ctx: &DockCtx) -> Option<Element<DockletMsg>> {
        use iced_native::*;

        let toplevels = ctx.toplevels.borrow();
        let appid = &self.app.id;
        while self.toplevels_buttons.len() < toplevels.values().len() {
            self.toplevels_buttons.push(Default::default());
        }
        let mut btns = Scrollable::new(&mut self.toplevels_scrollable).spacing(2);
        // ugh, fold results in closure lifetime issues
        for (i, topl) in toplevels
            .values()
            .filter(|topl| topl.matches_id(appid))
            .enumerate()
        {
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

    fn update(&mut self, ctx: &DockCtx, msg: DockletMsg) {
        let toplevels = ctx.toplevels.borrow();

        match msg {
            DockletMsg::App(Msg::ActivateApp) => {
                for topl in toplevels.values() {
                    if topl.matches_id(self.id()) {
                        topl.handle.activate(&ctx.seat);
                        return;
                    }
                }
                self.app
                    .info
                    .launch::<gio::AppLaunchContext>(&[], None)
                    .unwrap()
            }
            DockletMsg::App(Msg::ActivateToplevel(topli)) => {
                toplevels
                    .values()
                    .filter(|topl| topl.matches_id(self.id()))
                    .nth(topli)
                    .unwrap()
                    .handle
                    .activate(&ctx.seat);
            }
            _ => (),
        }
    }
}
