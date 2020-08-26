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

#[derive(Debug, Clone)]
pub enum DockletMsg {
    Hover,
    App(app::Msg),
}

#[derive(Debug, Clone)]
pub enum Msg {
    IdxMsg(usize, DockletMsg),
}

pub trait Docklet {
    fn widget(&mut self, running: bool) -> Element<DockletMsg>;
    fn width(&self) -> u16;
    fn overhang(&mut self, toplevels: ToplevelStates) -> Element<DockletMsg>;
    fn update(&mut self, toplevels: ToplevelStates, seat: &wl_seat::WlSeat, msg: app::Msg);
}

mod app;

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

// #[derive(Debug, Clone)]
// pub enum Evt {
// }

pub struct Dock {
    is_pointed: bool,
    is_touched: bool,
    seat: wl_seat::WlSeat,
    toplevels: ToplevelStates,
    apps: Vec<app::AppDocklet>,
    hovered_app: Option<usize>,
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
                if let Some(app) = app::AppDocklet::from_id(id) {
                    self.apps.push(app);
                }
            }
        }

        let toplevels = self.toplevels.borrow();
        for topl in toplevels.values() {
            if self.apps.iter().find(|a| topl.matches_id(a.id())).is_none() {
                if let Some(app) = app::AppDocklet::from_id(&topl.app_id).or_else(|| {
                    topl.gtk_app_id
                        .as_ref()
                        .and_then(|gid| app::AppDocklet::from_id(&gid))
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

    fn hovered_app(&self) -> Option<usize> {
        if self.is_pointed || self.is_touched {
            self.hovered_app
        } else {
            None
        }
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

        let apps_r = unsafe { &mut *(&mut self.apps as *mut Vec<app::AppDocklet>) }; // XXX multiple borrows
        let mut col = Column::new().width(Length::Fill);

        let dock_width = self.width();
        if let Some(appi) = self.hovered_app() {
            let our_center = self.center_of_app(appi);
            let i = self.apps[appi]
                .overhang(self.toplevels.clone())
                .map(move |m| Msg::IdxMsg(appi, m))
                .into();
            col = col.push(overhang(
                Length::Units(TOPLEVELS_WIDTH), // TODO remove this arg
                (dock_width as i16 / 2 - our_center as i16) * 2, // XXX: why is the *2 needed?
                i,
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
        let row = apps_r.iter_mut().enumerate().fold(
            Row::new().align_items(Align::Center).spacing(DOCK_PADDING),
            |row, (i, app)| {
                let running = toplevels.values().any(|topl| topl.matches_id(app.id()));
                row.push(app.widget(running).map(move |m| Msg::IdxMsg(i, m)))
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
        if let Some(appi) = self.hovered_app() {
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
            Msg::IdxMsg(appi, DockletMsg::Hover) => self.hovered_app = Some(appi),
            Msg::IdxMsg(i, DockletMsg::App(amsg)) => {
                self.apps[i].update(self.toplevels.clone(), &self.seat, amsg)
            }
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
