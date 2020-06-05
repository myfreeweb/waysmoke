use crate::{style, util::*};
use wstk::*;

lazy_static::lazy_static! {
    static ref UNKNOWN_ICON: icons::IconHandle =
        icons::IconHandle::from_path(apps::unknown_icon());
}

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
    pub fn new(app: apps::App) -> DockApp {
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

    pub fn from_id(id: &str) -> Option<DockApp> {
        apps::App::lookup(id).map(DockApp::new)
    }

    pub fn id(&self) -> &str {
        &self.app.id
    }

    pub fn widget(&mut self, position: usize) -> Element<Msg> {
        use iced_native::*;

        Container::new(
            Button::new(&mut self.button, self.icon.clone().widget())
                .style(style::Dock)
                .on_press(Msg::ActivateApp(position)),
        )
        .style(style::Dock)
        .center_x()
        .center_y()
        .into()
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
        layer_surface.set_size(0, 85);
        layer_surface.set_exclusive_zone(10);
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
                Row::new().align_items(Align::Center).spacing(4),
                |row, (i, app)| row.push(app.widget(i)),
            );
            // TODO: show toplevels for unrecognized apps

            let dock = Container::new(
                Container::new(row)
                    .style(style::Dock)
                    .width(Length::Shrink)
                    .height(Length::Units(69))
                    .center_x()
                    .center_y()
                    .padding(4),
            )
            .width(Length::Fill)
            .height(Length::Units(75))
            .center_x();

            col = col.push(dock);
        } else {
            col = col
                .push(Container::new(Text::new("".to_string()).size(0)).height(Length::Units(75)));
        }

        let bar = Container::new(
            Container::new(Text::new("".to_string()).size(0))
                .style(style::WhiteStripe)
                .width(Length::Units(128))
                .height(Length::Units(4)),
        )
        .style(style::DarkBar)
        .width(Length::Fill)
        .height(Length::Units(10))
        .center_x()
        .center_y();

        col.push(bar).into()
    }

    fn input_region(&self, width: i32, _height: i32) -> Option<Vec<Rectangle<i32>>> {
        let bar = Rectangle {
            x: 0,
            y: 75,
            width,
            height: 10,
        };
        if self.shown {
            let dock_width = 420; // TODO actually calculate based on icons
            Some(vec![
                Rectangle {
                    x: (width - dock_width) / 2,
                    y: 0,
                    width: dock_width,
                    height: 85,
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
