use crate::{style, svc, util::*};
use futures::prelude::*;
use gio::{AppInfoExt, DesktopAppInfoExt};
use std::{cell::Cell, collections::HashMap};
use wstk::*;

lazy_static::lazy_static! {
    static ref UNKNOWN_ICON: wstk::ImageHandle =
        icons::icon_from_path(apps::icon("application-x-executable"));
}

pub const POPOVER_HEIGHT_MAX: u16 = 420;
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

#[async_trait(?Send)]
pub trait Docklet {
    fn widget(&mut self) -> Element<DockletMsg>;
    fn width(&self) -> u16;
    fn retained_icon(&self) -> Option<wstk::ImageHandle> {
        None
    }
    fn popover(&mut self) -> Option<Element<DockletMsg>> {
        None
    }
    fn update(&mut self, msg: DockletMsg);
    async fn run(&mut self);
}

mod app;
mod power;

fn popover<'a>(
    icon_offset: i16,
    content: Element<'a, Msg>,
    popover_region: &'a Cell<Rectangle>,
) -> Element<'a, Msg> {
    use iced_graphics::{
        triangle::{Mesh2D, Vertex2D},
        Primitive,
    };
    use iced_native::*;

    let content_box = Container::new(content)
        .style(style::Dock(style::DARK_COLOR))
        .width(Length::Shrink)
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

    let content_col = GetRegion::new(
        popover_region,
        Column::new()
            .align_items(Align::Center)
            .push(content_box)
            .push(triangle),
    );

    let mut offset_row = Row::new().height(Length::Shrink);
    if icon_offset < 0 {
        offset_row = offset_row.push(Space::new(
            Length::Units(-icon_offset as _),
            Length::Units(0),
        ));
    }
    offset_row = offset_row.push(content_col);
    if icon_offset > 0 {
        offset_row = offset_row.push(Space::new(
            Length::Units(icon_offset as _),
            Length::Units(0),
        ));
    }
    Container::new(offset_row)
        .width(Length::Fill)
        .height(Length::Units(POPOVER_HEIGHT_MAX))
        .center_x()
        .align_y(Align::End)
        .into()
}

#[derive(Clone)]
pub struct DockCtx {
    pub seat: wl_seat::WlSeat,
    pub toplevel_updates: wstk::bus::Subscriber<
        HashMap<wstk::toplevels::ToplevelKey, wstk::toplevels::ToplevelState>,
    >,
    pub power_updates: wstk::bus::Subscriber<svc::power::PowerState>,
}

pub struct Dock {
    ctx: DockCtx,
    is_pointed: bool,
    is_touched: bool,
    hovered_docklet: Option<usize>,
    toplevel_updates: wstk::bus::Subscriber<
        HashMap<wstk::toplevels::ToplevelKey, wstk::toplevels::ToplevelState>,
    >,

    dock_region: Cell<Rectangle>,
    popover_region: Cell<Rectangle>,

    apps: Vec<app::AppDocklet>,
    power: power::PowerDocklet,
}

impl Dock {
    pub fn new(ctx: DockCtx) -> Dock {
        let power = power::PowerDocklet::new(ctx.power_updates.clone());
        let toplevel_updates = ctx.toplevel_updates.clone();
        Dock {
            ctx,
            is_pointed: false,
            is_touched: false,
            hovered_docklet: None,
            toplevel_updates,
            dock_region: Default::default(),
            popover_region: Default::default(),
            apps: Vec::new(),
            power,
        }
    }

    fn update_apps(
        &mut self,
        toplevels: HashMap<wstk::toplevels::ToplevelKey, wstk::toplevels::ToplevelState>,
    ) {
        self.hovered_docklet = None;

        let docked = vec![
            "firefox",
            "Alacritty",
            "org.gnome.Lollypop",
            "org.gnome.Nautilus",
            "telegramdesktop",
        ]; // TODO: GSettings

        for id in docked.iter() {
            if self.apps.iter().find(|a| a.id() == *id).is_none() {
                if let Some(app) = app::AppDocklet::from_id(
                    id,
                    self.ctx.seat.clone(),
                    self.ctx.toplevel_updates.clone(),
                ) {
                    self.apps.push(app);
                }
            }
        }

        for topl in toplevels.values() {
            if self.apps.iter().find(|a| topl.matches_id(a.id())).is_none() {
                if let Some(app) = app::AppDocklet::from_id(
                    &topl.app_id,
                    self.ctx.seat.clone(),
                    self.ctx.toplevel_updates.clone(),
                )
                .or_else(|| {
                    topl.gtk_app_id.as_ref().and_then(|gid| {
                        app::AppDocklet::from_id(
                            &gid,
                            self.ctx.seat.clone(),
                            self.ctx.toplevel_updates.clone(),
                        )
                    })
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

    fn docklets(&self) -> impl Iterator<Item = &dyn Docklet> {
        self.apps
            .iter()
            .map(|x| &*x as &dyn Docklet)
            .chain(std::iter::once(&self.power as &dyn Docklet))
    }

    fn width(&self) -> u16 {
        let (wid, cnt) = self
            .docklets()
            .fold((0, 0), |(w, c), d| (w + d.width(), c + 1));
        wid + DOCK_PADDING * (std::cmp::max(cnt as u16, 1) - 1) + DOCK_PADDING * 2
    }

    fn center_of_docklet(&self, id: usize) -> u16 {
        DOCK_PADDING
            + self
                .docklets()
                .take(id)
                .fold(0, |x, d| x + d.width() + DOCK_PADDING)
            + self.docklets().nth(id).unwrap().width() / 2
    }

    fn hovered_docklet(&self) -> Option<usize> {
        if self.is_pointed || self.is_touched {
            self.hovered_docklet
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
        layer_surface.set_size(
            0,
            (BAR_HEIGHT + DOCK_AND_GAP_HEIGHT + POPOVER_HEIGHT_MAX) as _,
        );
        layer_surface.set_exclusive_zone(BAR_HEIGHT as _);
    }
}

#[async_trait(?Send)]
impl IcedSurface for Dock {
    type Message = Msg;

    fn view(&mut self) -> Element<Self::Message> {
        use iced_native::*;

        let mut col = Column::new().width(Length::Fill);

        let dock_width = self.width();
        let mut has_oh = false;
        if let Some(appi) = self.hovered_docklet() {
            let our_center = self.center_of_docklet(appi);
            let docklet = self.docklets().nth(appi).unwrap();
            if let Some(oh) =
                unsafe { &mut *(docklet as *const dyn Docklet as *mut dyn Docklet) }.popover()
            {
                let i = oh.map(move |m| Msg::IdxMsg(appi, m)).into();
                col = col.push(popover(
                    (dock_width as i16 / 2 - our_center as i16) * 2, // XXX: why is the *2 needed?
                    i,
                    &self.popover_region,
                ));
                has_oh = true;
            }
        }
        if !has_oh {
            self.popover_region.set(Default::default()); // probably not the best way to clear input region but w/e
            col = col.push(Space::with_height(Length::Units(POPOVER_HEIGHT_MAX)));
        }

        if self.is_pointed || self.is_touched {
            let row = self.docklets().enumerate().fold(
                Row::new().align_items(Align::Center).spacing(DOCK_PADDING),
                |row, (i, docklet)| {
                    row.push(
                        unsafe { &mut *(docklet as *const dyn Docklet as *mut dyn Docklet) }
                            .widget()
                            .map(move |m| Msg::IdxMsg(i, m)),
                    )
                },
            );
            // TODO: show toplevels for unrecognized apps

            let dock = Container::new(
                GetRegion::new(
                    &self.dock_region,
                    Container::new(row)
                        .style(style::Dock(style::DARK_COLOR))
                        .width(Length::Shrink)
                        .height(Length::Shrink)
                        .center_x()
                        .center_y()
                        .padding(DOCK_PADDING),
                )
                .center_x()
                .center_y(),
            )
            .width(Length::Fill)
            .height(Length::Units(DOCK_HEIGHT))
            .center_x();

            col = col
                .push(dock)
                .push(Space::with_height(Length::Units(DOCK_GAP)));
        } else {
            col = col.push(Space::with_height(Length::Units(DOCK_AND_GAP_HEIGHT)));
        }

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

    fn input_region(&self, width: u32, _height: u32) -> Option<Vec<Rectangle<u32>>> {
        fn pad(rect: Rectangle<u32>, n: u32) -> Rectangle<u32> {
            if rect.x < n || rect.y < n {
                return rect;
            }
            Rectangle {
                x: rect.x - n,
                y: rect.y - n,
                width: rect.width + n * 2,
                height: rect.height + n * 2,
            }
        }
        let bar = Rectangle {
            x: 0,
            y: (DOCK_AND_GAP_HEIGHT + POPOVER_HEIGHT_MAX) as _,
            width,
            height: BAR_HEIGHT as _,
        };
        let mut result = vec![bar];
        if self.is_pointed || self.is_touched {
            result.push(pad(self.dock_region.get().snap(), 12));
            if self.hovered_docklet().is_some() {
                result.push(pad(self.popover_region.get().snap(), 6));
            }
        }
        Some(result)
    }

    fn retained_images(&mut self) -> Vec<wstk::ImageHandle> {
        self.docklets().flat_map(|d| d.retained_icon()).collect()
    }

    async fn update(&mut self, message: Self::Message) {
        match message {
            Msg::IdxMsg(i, DockletMsg::Hover) => self.hovered_docklet = Some(i),
            Msg::IdxMsg(i, dmsg) => {
                let docklet = self.docklets().nth(i).unwrap();
                unsafe { &mut *(docklet as *const dyn Docklet as *mut dyn Docklet) }.update(dmsg)
            }
        }
    }

    async fn run(&mut self) -> bool {
        // ARGH: avoiding multiple mutable borrows is so hard in a situation like this!
        //       even sel's Drop (!) mutably borrows self.apps, hence the clone/drop dance.
        //       also the 'docklets_mut' has to be inline - if it was a method, rustc couldn't
        //       see inside of it and find that it only touches self.apps and not the rest of self
        let sel = future::select(
            self.toplevel_updates.next(),
            future::select_all(
                self.apps
                    .iter_mut()
                    .map(|x| x.run())
                    .chain(std::iter::once(self.power.run())),
            ),
        )
        .await;
        if let future::Either::Left((Some(ref tt), _)) = sel {
            let xt = tt.clone();
            drop(sel);
            self.update_apps((*xt).clone());
        }
        true
    }

    async fn on_pointer_enter(&mut self) {
        self.is_pointed = true;
    }

    async fn on_pointer_leave(&mut self) {
        self.is_pointed = false;
        self.hovered_docklet = None;
    }

    async fn on_touch_enter(&mut self) {
        self.is_touched = true;
    }

    async fn on_touch_leave(&mut self) {
        self.is_touched = false;
    }
}
