use crate::{style, util::*};
use wstk::*;

lazy_static::lazy_static! {
    static ref UNKNOWN_ICON: icons::IconHandle =
        icons::IconHandle::from_path(apps::unknown_icon());
}

pub const APP_PADDING: u16 = 4;
pub const DOCK_PADDING: u16 = 4;
pub const DOCK_GAP: u16 = 6;
pub const BAR_HEIGHT: u16 = 10;
pub const DOCK_AND_GAP_HEIGHT: u16 =
    icons::ICON_SIZE + APP_PADDING * 2 + DOCK_PADDING * 2 + DOCK_GAP;

#[derive(Debug, Clone)]
pub enum Msg {
    ActivateApp(usize),
}

// #[derive(Debug, Clone)]
// pub enum Evt {
// }

struct DockApp {
    app: apps::App,
    icon: icons::IconHandle,
    button: iced_native::button::State,
}

impl DockApp {
    fn new(app: apps::App) -> DockApp {
        let icon = app
            .icon()
            .map(icons::IconHandle::from_path)
            .unwrap_or_else(|| UNKNOWN_ICON.clone());
        DockApp {
            app,
            icon,
            button: Default::default(),
        }
    }

    fn from_id(id: &str) -> Option<DockApp> {
        apps::App::lookup(id).map(DockApp::new)
    }

    fn id(&self) -> &str {
        &self.app.id
    }

    fn widget(&mut self, position: usize) -> Element<Msg> {
        use iced_native::*;

        let big_button = Button::new(&mut self.button, self.icon.clone().widget())
            .style(style::Dock)
            .padding(APP_PADDING)
            .on_press(Msg::ActivateApp(position));

        Container::new(big_button)
            .style(style::Dock)
            .center_x()
            .center_y()
            .into()
    }

    fn width(&self) -> u16 {
        // TODO: will be dynamic based on extras
        icons::ICON_SIZE + APP_PADDING * 2
    }
}

pub struct Dock {
    shown: bool,
    seat: wl_seat::WlSeat,
    toplevels: ToplevelStates,
    apps: Vec<DockApp>,
}

impl Dock {
    pub fn new(seat: wl_seat::WlSeat, toplevels: ToplevelStates) -> Dock {
        Dock {
            shown: false,
            seat,
            toplevels,
            apps: Vec::new(),
        }
    }

    fn update_apps(&mut self) {
        let docked = vec!["Nightly", "Alacritty", "org.gnome.Lollypop"]; // TODO: GSettings

        for id in docked.iter() {
            if self.apps.iter().find(|a| a.id() == *id).is_none() {
                if let Some(app) = DockApp::from_id(id) {
                    self.apps.push(app);
                }
            }
        }

        let toplevels = self.toplevels.borrow();
        for topl in toplevels.values() {
            if self.apps.iter().find(|a| topl.matches_id(a.id())).is_none() {
                if let Some(app) = DockApp::from_id(&topl.app_id).or_else(|| {
                    topl.gtk_app_id
                        .as_ref()
                        .and_then(|gid| DockApp::from_id(&gid))
                }) {
                    self.apps.push(app);
                }
            }
        }

        self.apps.retain(|a| {
            docked.iter().any(|id| a.id() == *id)
                || toplevels.values().any(|topl| topl.matches_id(a.id()))
        });
    }
}

impl DesktopWidget for Dock {
    fn setup_lsh(&self, layer_surface: &Main<layer_surface::ZwlrLayerSurfaceV1>) {
        layer_surface.set_anchor(
            layer_surface::Anchor::Left
                | layer_surface::Anchor::Right
                | layer_surface::Anchor::Bottom,
        );
        layer_surface.set_size(0, (BAR_HEIGHT + DOCK_AND_GAP_HEIGHT) as _);
        layer_surface.set_exclusive_zone(BAR_HEIGHT as _);
    }
}

#[async_trait(?Send)]
impl IcedWidget for Dock {
    type Message = Msg;
    type ExternalEvent = ();

    fn view(&mut self) -> Element<Self::Message> {
        use iced_native::*;

        let mut col = Column::new().width(Length::Fill);

        if self.shown {
            let row = self.apps.iter_mut().enumerate().fold(
                Row::new().align_items(Align::Center).spacing(DOCK_PADDING),
                |row, (i, app)| row.push(app.widget(i)),
            );
            // TODO: show toplevels for unrecognized apps

            let dock = Container::new(
                Container::new(row)
                    .style(style::Dock)
                    .width(Length::Shrink)
                    .height(Length::Shrink)
                    .center_x()
                    .center_y()
                    .padding(DOCK_PADDING),
            )
            .width(Length::Fill)
            .height(Length::Shrink)
            .center_x();

            col = col.push(dock).push(
                Container::new(Text::new("".to_string()).size(0)).height(Length::Units(DOCK_GAP)),
            );
        } else {
            col = col.push(
                Container::new(Text::new("".to_string()).size(0))
                    .height(Length::Units(DOCK_AND_GAP_HEIGHT)),
            );
        }

        let bar = Container::new(
            Container::new(Text::new("".to_string()).size(0))
                .style(style::WhiteStripe)
                .width(Length::Units(128))
                .height(Length::Units(4)),
        )
        .style(style::DarkBar)
        .width(Length::Fill)
        .height(Length::Units(BAR_HEIGHT))
        .center_x()
        .center_y();

        col.push(bar).into()
    }

    fn input_region(&self, width: i32, _height: i32) -> Option<Vec<Rectangle<i32>>> {
        let bar = Rectangle {
            x: 0,
            y: DOCK_AND_GAP_HEIGHT as _,
            width,
            height: BAR_HEIGHT as _,
        };
        if self.shown {
            let dock_width = (self.apps.iter().fold(0, |w, app| w + app.width())
                + DOCK_PADDING * (std::cmp::max(self.apps.len() as u16, 1) - 1)
                + DOCK_PADDING * 2) as _;
            Some(vec![
                Rectangle {
                    x: (width - dock_width) / 2,
                    y: 0,
                    width: dock_width,
                    height: (BAR_HEIGHT + DOCK_AND_GAP_HEIGHT) as _,
                },
                bar,
            ])
        } else {
            Some(vec![bar])
        }
    }

    async fn update(&mut self, message: Self::Message) {
        use gio::AppInfoExt;
        match message {
            Msg::ActivateApp(id) => {
                let toplevels = self.toplevels.borrow();
                for topl in toplevels.values() {
                    if topl.matches_id(self.apps[id].id()) {
                        topl.handle.activate(&self.seat);
                        return;
                    }
                }
                self.apps[id]
                    .app
                    .info
                    .launch::<gio::AppLaunchContext>(&[], None)
                    .unwrap()
            }
        }
    }

    async fn react(&mut self, _event: Self::ExternalEvent) {
        self.update_apps();
    }

    async fn on_pointer_enter(&mut self) {
        self.shown = true;
    }

    async fn on_pointer_leave(&mut self) {
        self.shown = false;
    }
}
