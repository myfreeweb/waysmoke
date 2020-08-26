use crate::{style, util::*};
use gio::{AppInfoExt, DesktopAppInfoExt};
use wstk::*;

lazy_static::lazy_static! {
    static ref UNKNOWN_ICON: icons::IconHandle =
        icons::IconHandle::from_path(apps::unknown_icon());
}

pub const OVERHANG_HEIGHT: u16 = 420;
pub const TOPLEVELS_WIDTH: u16 = 290;
pub const APP_PADDING: u16 = 4;
pub const DOCK_PADDING: u16 = 4;
pub const DOCK_GAP: u16 = 8;
pub const BAR_HEIGHT: u16 = 10;
pub const DOCK_HEIGHT: u16 = icons::ICON_SIZE + APP_PADDING * 2 + DOCK_PADDING * 2;
pub const DOCK_AND_GAP_HEIGHT: u16 = DOCK_HEIGHT + DOCK_GAP;

fn overhang(width: iced_native::Length, icon_offset: i16, content: Element<Msg>) -> Element<Msg> {
    use iced_graphics::{
        triangle::{Mesh2D, Vertex2D},
        Primitive,
    };
    use iced_native::*;

    let content_box = Container::new(content)
        .style(style::Dock(style::DARK_COLOR))
        .width(Length::Fill)
        .padding(DOCK_PADDING);

    let triangle = prim::Prim::new(Primitive::Mesh2D {
        buffers: Mesh2D {
            vertices: vec![
                Vertex2D {
                    position: [0.0, 0.0],
                    color: style::DARK_COLOR.into_linear(),
                },
                Vertex2D {
                    position: [8.0, 8.0],
                    color: style::DARK_COLOR.into_linear(),
                },
                Vertex2D {
                    position: [16.0, 0.0],
                    color: style::DARK_COLOR.into_linear(),
                },
            ],
            indices: vec![0, 1, 2],
        },
        size: iced_graphics::Size::new(16.0, 8.0),
    })
    .width(Length::Units(16))
    .height(Length::Units(8));

    let content_col = Column::new()
        .align_items(Align::Center)
        .width(width)
        .push(content_box)
        .push(triangle);

    let mut offset_row = Row::new().height(Length::Shrink);
    if icon_offset < 0 {
        offset_row = offset_row.push(
            prim::Prim::new(Primitive::None)
                .height(Length::Units(0))
                .width(Length::Units(-icon_offset as _)),
        );
    }
    offset_row = offset_row.push(content_col);
    if icon_offset > 0 {
        offset_row = offset_row.push(
            prim::Prim::new(Primitive::None)
                .height(Length::Units(0))
                .width(Length::Units(icon_offset as _)),
        );
    }
    Container::new(offset_row)
        .width(Length::Fill)
        .height(Length::Units(OVERHANG_HEIGHT))
        .center_x()
        .align_y(Align::End)
        .into()
}

#[derive(Debug, Clone)]
pub enum Msg {
    ActivateApp(usize),
    ActivateToplevel(usize, usize),
    HoverApp(usize),
}

// #[derive(Debug, Clone)]
// pub enum Evt {
// }

struct DockApp {
    app: apps::App,
    icon: icons::IconHandle,
    button: iced_native::button::State,
    evl: addeventlistener::State,
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
            evl: Default::default(),
        }
    }

    fn from_id(id: &str) -> Option<DockApp> {
        apps::App::lookup(id).map(DockApp::new)
    }

    fn id(&self) -> &str {
        &self.app.id
    }

    fn widget(&mut self, position: usize, running: bool) -> Element<Msg> {
        use iced_native::*;

        let big_button = Button::new(&mut self.button, self.icon.clone().widget())
            .style(style::Dock(style::DARK_COLOR))
            .padding(APP_PADDING)
            .on_press(Msg::ActivateApp(position));

        let listener = AddEventListener::new(&mut self.evl, big_button)
            .on_pointer_enter(Msg::HoverApp(position));

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
}

pub struct Dock {
    is_pointed: bool,
    is_touched: bool,
    seat: wl_seat::WlSeat,
    toplevels: ToplevelStates,
    apps: Vec<DockApp>,
    hovered_app: Option<usize>,
    toplevels_scrollable: iced_native::scrollable::State,
    toplevels_buttons: Vec<iced_native::button::State>,
}

impl Dock {
    pub fn new(seat: wl_seat::WlSeat, toplevels: ToplevelStates) -> Dock {
        Dock {
            is_pointed: false,
            is_touched: false,
            seat,
            toplevels,
            apps: Vec::new(),
            hovered_app: None,
            toplevels_scrollable: Default::default(),
            toplevels_buttons: Default::default(),
        }
    }

    fn update_apps(&mut self) {
        self.hovered_app = None;

        let docked = vec![
            "firefox",
            "Alacritty",
            "org.gnome.Lollypop",
            "org.gnome.Nautilus",
            "telegramdesktop",
        ]; // TODO: GSettings

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

    fn width(&self) -> u16 {
        self.apps.iter().fold(0, |w, app| w + app.width())
            + DOCK_PADDING * (std::cmp::max(self.apps.len() as u16, 1) - 1)
            + DOCK_PADDING * 2
    }

    fn center_of_app(&self, id: usize) -> u16 {
        DOCK_PADDING
            + self.apps[..id]
                .iter()
                .fold(0, |acc, app| acc + app.width() + DOCK_PADDING)
            + self.apps[id].width() / 2
    }
}

impl DesktopSurface for Dock {
    fn setup_lsh(&self, layer_surface: &Main<layer_surface::ZwlrLayerSurfaceV1>) {
        layer_surface.set_anchor(
            layer_surface::Anchor::Left
                | layer_surface::Anchor::Right
                | layer_surface::Anchor::Bottom,
        );
        layer_surface.set_size(0, (BAR_HEIGHT + DOCK_AND_GAP_HEIGHT + OVERHANG_HEIGHT) as _);
        layer_surface.set_exclusive_zone(BAR_HEIGHT as _);
    }
}

#[async_trait(?Send)]
impl IcedSurface for Dock {
    type Message = Msg;
    type ExternalEvent = ();

    fn view(&mut self) -> Element<Self::Message> {
        use iced_native::*;

        let mut col = Column::new().width(Length::Fill);

        let dock_width = self.width();
        if let Some(appi) = if self.is_pointed || self.is_touched {
            self.hovered_app
        } else {
            None
        } {
            let our_center = self.center_of_app(appi);
            let toplevels = self.toplevels.borrow();
            let appid = self.apps[appi].id();
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
                    .on_press(Msg::ActivateToplevel(appi, i)),
                )
            }
            let title = Text::new(
                self.apps[appi]
                    .app
                    .info
                    .get_name()
                    .map(|x| x.to_owned())
                    .unwrap_or("<untitled>".to_owned()),
            )
            .width(Length::Fill)
            .horizontal_alignment(HorizontalAlignment::Center)
            .size(16);
            col = col.push(overhang(
                Length::Units(TOPLEVELS_WIDTH),
                (dock_width as i16 / 2 - our_center as i16) * 2, // XXX: why is the *2 needed?
                Column::new()
                    .push(title)
                    .push(btns)
                    .spacing(DOCK_PADDING)
                    .into(),
            ));
        } else {
            col = col.push(
                prim::Prim::new(iced_graphics::Primitive::None)
                    .height(Length::Units(OVERHANG_HEIGHT)),
            );
        }

        // XXX: removing the icons from the output causes them to be unloaded
        //      so for now we just make the dock invisible

        // if self.is_pointed || self.is_touched {
        let toplevels = self.toplevels.borrow();
        let row = self.apps.iter_mut().enumerate().fold(
            Row::new().align_items(Align::Center).spacing(DOCK_PADDING),
            |row, (i, app)| {
                let running = toplevels.values().any(|topl| topl.matches_id(app.id()));
                row.push(app.widget(i, running))
            },
        );
        // TODO: show toplevels for unrecognized apps

        let dock = Container::new(
            Container::new(row)
                .style(style::Dock(style::DARK_COLOR))
                .width(Length::Shrink)
                .height(Length::Shrink)
                .center_x()
                .center_y()
                .padding(if self.is_pointed || self.is_touched {
                    DOCK_PADDING
                } else {
                    0
                }),
        )
        .width(if self.is_pointed || self.is_touched {
            Length::Fill
        } else {
            Length::Units(0)
        })
        .height(Length::Units(DOCK_HEIGHT))
        .center_x();

        col = col
            .push(dock)
            .push(prim::Prim::new(iced_graphics::Primitive::None).height(Length::Units(DOCK_GAP)));
        // } else {
        //     col = col.push(
        //         prim::Prim::new(iced_graphics::Primitive::None)
        //             .height(Length::Units(DOCK_AND_GAP_HEIGHT)),
        //     );
        // }

        let bar = Container::new(
            prim::Prim::new(iced_graphics::Primitive::Quad {
                bounds: iced_graphics::Rectangle::with_size(Size::new(192.0, 4.0)),
                background: Background::Color(Color::WHITE),
                border_radius: 2,
                border_width: 0,
                border_color: Color::WHITE,
            })
            .width(Length::Units(192))
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
            y: (DOCK_AND_GAP_HEIGHT + OVERHANG_HEIGHT) as _,
            width,
            height: BAR_HEIGHT as _,
        };
        let mut result = vec![bar];
        if self.is_pointed || self.is_touched {
            let dock_width = self.width() as _;
            result.push(Rectangle {
                x: (width - dock_width) / 2,
                y: OVERHANG_HEIGHT as _,
                width: dock_width,
                height: (BAR_HEIGHT + DOCK_AND_GAP_HEIGHT) as _,
            });
        }
        if let Some(appi) = if self.is_pointed || self.is_touched {
            self.hovered_app
        } else {
            None
        } {
            let toplevels_height = 200; // TODO: calc
            let dock_width = self.width() as i32;
            let our_center = self.center_of_app(appi) as i32;
            result.push(Rectangle {
                x: (width - TOPLEVELS_WIDTH as i32) / 2 - (dock_width / 2 - our_center),
                y: (OVERHANG_HEIGHT - toplevels_height) as _,
                width: TOPLEVELS_WIDTH as _,
                height: toplevels_height as _,
            });
        }
        Some(result)
    }

    async fn update(&mut self, message: Self::Message) {
        match message {
            Msg::ActivateApp(appi) => {
                let toplevels = self.toplevels.borrow();
                for topl in toplevels.values() {
                    if topl.matches_id(self.apps[appi].id()) {
                        topl.handle.activate(&self.seat);
                        return;
                    }
                }
                self.apps[appi]
                    .app
                    .info
                    .launch::<gio::AppLaunchContext>(&[], None)
                    .unwrap()
            }
            Msg::ActivateToplevel(appi, topli) => {
                let toplevels = self.toplevels.borrow();
                toplevels
                    .values()
                    .filter(|topl| topl.matches_id(self.apps[appi].id()))
                    .nth(topli)
                    .unwrap()
                    .handle
                    .activate(&self.seat);
            }
            Msg::HoverApp(appi) => self.hovered_app = Some(appi),
        }
    }

    async fn react(&mut self, _event: Self::ExternalEvent) {
        self.update_apps();
    }

    async fn on_pointer_enter(&mut self) {
        self.is_pointed = true;
    }

    async fn on_pointer_leave(&mut self) {
        self.is_pointed = false;
        self.hovered_app = None;
    }

    async fn on_touch_enter(&mut self) {
        self.is_touched = true;
    }

    async fn on_touch_leave(&mut self) {
        self.is_touched = false;
    }
}
