use crate::{dock::*, style};
use std::sync::Arc;

lazy_static::lazy_static! {
    static ref PLAY_ICON: wstk::ImageHandle =
        icons::icon_from_path(apps::icon("media-playback-start-symbolic"));
    static ref PAUSE_ICON: wstk::ImageHandle =
        icons::icon_from_path(apps::icon("media-playback-pause-symbolic"));
    // static ref NEXT_ICON: wstk::ImageHandle =
    //     icons::icon_from_path(apps::icon("media-skip-forward-symbolic"));
}

#[derive(Debug, Clone)]
pub enum Msg {
    ActivateApp,
    ActivateToplevel(usize),
    MediaControl(usize, &'static str),
}

#[derive(Default)]
pub struct MediaBtns {
    play: iced_native::button::State,
    pause: iced_native::button::State,
}

pub struct AppDocklet {
    pub app: apps::App,
    pub icon: wstk::ImageHandle,
    pub button: iced_native::button::State,
    pub evl: addeventlistener::State,
    pub toplevels_scrollable: iced_native::scrollable::State,
    pub toplevels_buttons: Vec<iced_native::button::State>,
    pub seat: wl_seat::WlSeat,
    pub toplevel_updates: wstk::bus::Subscriber<
        HashMap<wstk::toplevels::ToplevelKey, wstk::toplevels::ToplevelState>,
    >,
    pub toplevels: Arc<HashMap<wstk::toplevels::ToplevelKey, wstk::toplevels::ToplevelState>>,
    pub media_svc: Arc<svc::media::MediaService>,
    pub media_updates: wstk::bus::Subscriber<svc::media::MediaState>,
    pub medias: Arc<svc::media::MediaState>,
    pub media_buttons: Vec<MediaBtns>,
}

impl AppDocklet {
    pub fn new(
        app: apps::App,
        seat: wl_seat::WlSeat,
        toplevel_updates: wstk::bus::Subscriber<
            HashMap<wstk::toplevels::ToplevelKey, wstk::toplevels::ToplevelState>,
        >,
        media_svc: Arc<svc::media::MediaService>,
        media_updates: wstk::bus::Subscriber<svc::media::MediaState>,
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
            toplevel_updates,
            toplevels: Arc::new(HashMap::new()),
            media_svc,
            media_updates,
            medias: Arc::new(HashMap::new()),
            media_buttons: Default::default(),
        }
    }

    pub fn id(&self) -> &str {
        &self.app.id
    }

    pub fn from_id(
        id: &str,
        seat: wl_seat::WlSeat,
        toplevel_updates: wstk::bus::Subscriber<
            HashMap<wstk::toplevels::ToplevelKey, wstk::toplevels::ToplevelState>,
        >,
        media_svc: Arc<svc::media::MediaService>,
        media_updates: wstk::bus::Subscriber<svc::media::MediaState>,
    ) -> Option<AppDocklet> {
        apps::App::lookup(id)
            .map(|a| AppDocklet::new(a, seat, toplevel_updates, media_svc, media_updates))
    }
}

#[async_trait(?Send)]
impl Docklet for AppDocklet {
    fn widget(&mut self) -> Element<DockletMsg> {
        use iced_native::*;

        let running = our_toplevels(&self.toplevels, self.id()).next().is_some();

        let big_button = Button::new(
            &mut self.button,
            icons::icon_widget(self.icon.clone(), ICON_SIZE),
        )
        .style(style::Dock(style::DARK_COLOR))
        .padding(APP_PADDING)
        .on_press(DockletMsg::App(Msg::ActivateApp));

        let mut content = Row::new().push(big_button);

        while self.media_buttons.len() < our_medias(&self.medias, &self.app.id).count() {
            self.media_buttons.push(Default::default());
        }
        for (i, ((_, media_data), btns)) in our_medias(&self.medias, &self.app.id)
            .zip(self.media_buttons.iter_mut())
            .enumerate()
        {
            content = content.push(
                if media_data.status == svc::media::PlaybackStatus::Playing {
                    Button::new(
                        &mut btns.pause,
                        Container::new(icons::icon_widget(PAUSE_ICON.clone(), ICON_SIZE / 2))
                            .height(Length::Fill)
                            .align_y(Align::Center),
                    )
                    .on_press(DockletMsg::App(Msg::MediaControl(i, "Pause")))
                } else {
                    Button::new(
                        &mut btns.play,
                        Container::new(icons::icon_widget(PLAY_ICON.clone(), ICON_SIZE / 2))
                            .height(Length::Fill)
                            .align_y(Align::Center),
                    )
                    .on_press(DockletMsg::App(Msg::MediaControl(i, "Play")))
                }
                .style(style::Toplevel)
                .padding(APP_PADDING)
                .height(Length::Fill),
            );
        }

        let listener =
            AddEventListener::new(&mut self.evl, content).on_pointer_enter(DockletMsg::Hover);

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
        ICON_SIZE
            + APP_PADDING * 2
            + our_medias(&self.medias, &self.app.id)
                .fold(0, |acc, _| acc + ICON_SIZE / 2 + APP_PADDING * 2)
    }

    fn retained_icon(&self) -> Option<wstk::ImageHandle> {
        Some(self.icon.clone())
    }

    fn popover(&mut self) -> Option<Element<DockletMsg>> {
        use iced_native::*;

        while self.toplevels_buttons.len() < our_toplevels(&self.toplevels, &self.app.id).count() {
            self.toplevels_buttons.push(Default::default());
        }
        let mut btns = Scrollable::new(&mut self.toplevels_scrollable).spacing(2);
        for (i, (topl, btn)) in our_toplevels(&self.toplevels, &self.app.id)
            .zip(self.toplevels_buttons.iter_mut())
            .enumerate()
        {
            btns = btns.push(
                Button::new(btn, Text::new(topl.title.clone()).size(14))
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
            DockletMsg::App(Msg::MediaControl(medi, op)) => {
                self.media_svc.control_player(
                    our_medias(&self.medias, &self.app.id).nth(medi).unwrap().0,
                    op,
                );
            }
            _ => (),
        }
    }

    async fn run(&mut self) {
        let this = self;
        futures::select! {
            tls = this.toplevel_updates.next().fuse() => this.toplevels = tls.unwrap(),
            med = this.media_updates.next().fuse() => this.medias = med.unwrap(),
        };
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

fn our_medias<'a>(
    medias: &'a Arc<svc::media::MediaState>,
    id: &'a String,
) -> impl Iterator<Item = (&'a String, &'a svc::media::MediaPlayerState)> {
    medias
        .iter()
        .filter(move |(_, m)| m.desktop_entry.as_ref() == Some(id) && m.can_pause && m.can_play)
}
