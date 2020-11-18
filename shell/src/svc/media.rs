use futures::prelude::*;
use gio::prelude::*;
use std::{
    cell::{Ref, RefCell},
    collections::HashMap,
    rc::Rc,
};
use wstk::event_listener;

#[derive(Debug, Clone, PartialEq)]
pub enum PlaybackStatus {
    Playing,
    Paused,
    Stopped,
}

impl PlaybackStatus {
    fn parse(x: &str) -> Option<PlaybackStatus> {
        match x {
            "Playing" => Some(PlaybackStatus::Playing),
            "Paused" => Some(PlaybackStatus::Paused),
            "Stopped" => Some(PlaybackStatus::Stopped),
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct MediaPlayerState {
    pub desktop_entry: Option<String>,
    pub status: PlaybackStatus,
    pub can_prev: bool,
    pub can_next: bool,
    pub can_play: bool,
    pub can_pause: bool,
}

impl MediaPlayerState {
    fn query(common: &gio::DBusProxy, player: &gio::DBusProxy) -> Option<MediaPlayerState> {
        Some(MediaPlayerState {
            desktop_entry: common.get_cached_property("DesktopEntry").and_then(|e| e.get()),
            status: player
                .get_cached_property("PlaybackStatus")
                .and_then(|e| e.get::<String>())
                .and_then(|e| PlaybackStatus::parse(&e))?,
            can_prev: player.get_cached_property("CanGoPrevious").and_then(|e| e.get())?,
            can_next: player.get_cached_property("CanGoNext").and_then(|e| e.get())?,
            can_play: player.get_cached_property("CanPlay").and_then(|e| e.get())?,
            can_pause: player.get_cached_property("CanPause").and_then(|e| e.get())?,
        })
    }

    fn common_update(&mut self, new_props: HashMap<String, glib::Variant>) {
        if let Some(e) = new_props.get("DesktopEntry").and_then(|e| e.get()) {
            self.desktop_entry = e;
        }
    }

    fn player_update(&mut self, new_props: HashMap<String, glib::Variant>) {
        if let Some(e) = new_props
            .get("PlaybackStatus")
            .and_then(|e| e.get::<String>())
            .and_then(|e| PlaybackStatus::parse(&e))
        {
            self.status = e;
        }
        if let Some(e) = new_props.get("CanGoPrevious").and_then(|e| e.get()) {
            self.can_prev = e;
        }
        if let Some(e) = new_props.get("CanGoNext").and_then(|e| e.get()) {
            self.can_next = e;
        }
        if let Some(e) = new_props.get("CanPlay").and_then(|e| e.get()) {
            self.can_play = e;
        }
        if let Some(e) = new_props.get("CanPause").and_then(|e| e.get()) {
            self.can_pause = e;
        }
    }
}

pub type MediaState = HashMap<String, MediaPlayerState>;

struct MediaSubscription {
    common: gio::DBusProxy,
    common_sub: Option<glib::SignalHandlerId>, // not cloneable in gio lol
    player: gio::DBusProxy,
    player_sub: Option<glib::SignalHandlerId>, // not cloneable in gio lol
}

impl Drop for MediaSubscription {
    fn drop(&mut self) {
        self.common.disconnect(self.common_sub.take().unwrap());
        self.player.disconnect(self.player_sub.take().unwrap());
    }
}

pub struct MediaService {
    dbus: gio::DBusConnection,
    noc_sub: Option<gio::SignalSubscriptionId>, // not cloneable in gio lol
    notifier: Rc<event_listener::Event>,
    state: Rc<RefCell<MediaState>>,
}

impl MediaService {
    pub async fn new(dbus: &gio::DBusConnection) -> MediaService {
        let notifier = Rc::new(event_listener::Event::new());
        let state = Rc::new(RefCell::new(HashMap::new()));
        let subs = Rc::new(RefCell::new(HashMap::new()));

        let names = dbus
            .send_message_with_reply_future(
                &gio::DBusMessage::new_method_call(
                    Some("org.freedesktop.DBus"),
                    "/org/freedesktop/DBus",
                    Some("org.freedesktop.DBus"),
                    "ListNames",
                ),
                gio::DBusSendMessageFlags::NONE,
                69,
            )
            .await
            .unwrap()
            .get_body()
            .unwrap()
            .get::<(Vec<String>,)>()
            .unwrap()
            .0;

        for name in names.into_iter().filter(|n| n.starts_with("org.mpris.")) {
            Self::add(dbus.clone(), notifier.clone(), state.clone(), subs.clone(), name).await;
        }

        // Oddly, GIO's higher level name-watching does not support arg0namespace, only exact names
        let noc_sub = {
            let notifier = notifier.clone();
            let state = state.clone();
            let dbus1 = dbus.clone();
            dbus.signal_subscribe(
                Some("org.freedesktop.DBus"),
                Some("org.freedesktop.DBus"),
                Some("NameOwnerChanged"),
                Some("/org/freedesktop/DBus"),
                Some("org.mpris"),
                gio::DBusSignalFlags::MATCH_ARG0_NAMESPACE,
                move |_bus, _, _, _, _, val| {
                    let (name, olduniq, newuniq) = val.get::<(String, String, String)>().unwrap();
                    if newuniq.is_empty() {
                        let _ = subs.borrow_mut().remove(&name);
                        let _ = state.borrow_mut().remove(&name);
                        notifier.notify(usize::MAX);
                    } else if olduniq.is_empty() {
                        glib::MainContext::default().spawn_local(Self::add(
                            dbus1.clone(),
                            notifier.clone(),
                            state.clone(),
                            subs.clone(),
                            name,
                        ));
                    }
                },
            )
        };

        MediaService {
            dbus: dbus.clone(),
            noc_sub: Some(noc_sub),
            notifier,
            state,
        }
    }

    async fn add(
        dbus: gio::DBusConnection,
        notifier: Rc<event_listener::Event>,
        state: Rc<RefCell<MediaState>>,
        subs: Rc<RefCell<HashMap<String, MediaSubscription>>>,
        name: String,
    ) {
        if let (Ok(common), Ok(player)) = (
            gio::DBusProxy::new_future(
                &dbus,
                gio::DBusProxyFlags::NONE,
                None,
                Some(&name),
                "/org/mpris/MediaPlayer2",
                "org.mpris.MediaPlayer2",
            )
            .await,
            gio::DBusProxy::new_future(
                &dbus,
                gio::DBusProxyFlags::NONE,
                None,
                Some(&name),
                "/org/mpris/MediaPlayer2",
                "org.mpris.MediaPlayer2.Player",
            )
            .await,
        ) {
            if let Some(initial_state) = MediaPlayerState::query(&common, &player) {
                let common_sub = {
                    let state = state.clone();
                    let name = name.clone();
                    let notifier = notifier.clone();
                    common
                        .connect_local("g-properties-changed", true, move |args| {
                            let new_props = args[1]
                                .get::<glib::Variant>()
                                .unwrap()
                                .unwrap()
                                .get::<HashMap<String, glib::Variant>>()
                                .unwrap();
                            if let Some(ref mut obj) = state.borrow_mut().get_mut(&name) {
                                obj.common_update(new_props);
                            }
                            notifier.notify(usize::MAX);
                            None
                        })
                        .ok()
                };
                let player_sub = {
                    let state = state.clone();
                    let name = name.clone();
                    let notifier = notifier.clone();
                    player
                        .connect_local("g-properties-changed", true, move |args| {
                            let new_props = args[1]
                                .get::<glib::Variant>()
                                .unwrap()
                                .unwrap()
                                .get::<HashMap<String, glib::Variant>>()
                                .unwrap();
                            if let Some(ref mut obj) = state.borrow_mut().get_mut(&name) {
                                obj.player_update(new_props);
                            }
                            notifier.notify(usize::MAX);
                            None
                        })
                        .ok()
                };
                state.borrow_mut().insert(name.clone(), initial_state);
                subs.borrow_mut().insert(
                    name,
                    MediaSubscription {
                        common,
                        common_sub,
                        player,
                        player_sub,
                    },
                );
                notifier.notify(usize::MAX);
            }
        } else {
            eprintln!("Failed to get proxies for MPRIS: {}", name);
        }
    }

    pub fn state(&self) -> Ref<'_, MediaState> {
        self.state.borrow()
    }

    pub fn subscribe(&self) -> impl Future<Output = ()> {
        self.notifier.listen()
    }

    pub fn control_player(&self, name: &str, cmd: &str) {
        let _ = self.dbus.send_message(
            &gio::DBusMessage::new_method_call(
                Some(name),
                "/org/mpris/MediaPlayer2",
                Some("org.mpris.MediaPlayer2.Player"),
                cmd,
            ),
            gio::DBusSendMessageFlags::NONE,
        );
    }
}

// not that it would ever be dropped but
impl Drop for MediaService {
    fn drop(&mut self) {
        self.dbus.signal_unsubscribe(self.noc_sub.take().unwrap());
    }
}
