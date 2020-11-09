use futures::prelude::*;
use gio::prelude::*;
use std::{cell::RefCell, collections::HashMap, sync::Arc};
use wstk::bus;

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
            desktop_entry: common
                .get_cached_property("DesktopEntry")
                .and_then(|e| e.get()),
            status: player
                .get_cached_property("PlaybackStatus")
                .and_then(|e| e.get::<String>())
                .and_then(|e| PlaybackStatus::parse(&e))?,
            can_prev: player
                .get_cached_property("CanGoPrevious")
                .and_then(|e| e.get())?,
            can_next: player
                .get_cached_property("CanGoNext")
                .and_then(|e| e.get())?,
            can_play: player
                .get_cached_property("CanPlay")
                .and_then(|e| e.get())?,
            can_pause: player
                .get_cached_property("CanPause")
                .and_then(|e| e.get())?,
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
    rx: bus::Subscriber<MediaState>,
}

impl MediaService {
    pub async fn new(dbus: &gio::DBusConnection) -> MediaService {
        let (tx, rx) = bus::bounded(1);
        let atx = Arc::new(RefCell::new(tx));
        let cur_state = Arc::new(RefCell::new((
            HashMap::<String, MediaPlayerState>::new(),
            HashMap::<String, MediaSubscription>::new(),
        )));

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
            Self::add(dbus, atx.clone(), cur_state.clone(), name).await;
        }

        // {
        //                     let mut stref = cur_state.borrow_mut();
        //         // atx.borrow_mut().send(.clone()).await.unwrap();
        // }

        // Oddly, GIO's higher level name-watching does not support arg0namespace, only exact names
        let dbus = dbus.clone();
        let dbus1 = dbus.clone();
        let noc_sub = dbus.signal_subscribe(
            Some("org.freedesktop.DBus"),
            Some("org.freedesktop.DBus"),
            Some("NameOwnerChanged"),
            Some("/org/freedesktop/DBus"),
            Some("org.mpris"),
            gio::DBusSignalFlags::MATCH_ARG0_NAMESPACE,
            move |_bus, _, _, _, _, val| {
                let (name, olduniq, newuniq) = val.get::<(String, String, String)>().unwrap();
                let cur_state = cur_state.clone();
                let atx = atx.clone();
                let dbus = dbus1.clone();
                glib::MainContext::default().spawn_local(async move {
                    if newuniq.is_empty() {
                        let mut stref = cur_state.borrow_mut();
                        let _ = stref.0.remove(&name);
                        let _ = stref.1.remove(&name);
                        atx.borrow_mut().send(stref.0.clone()).await.unwrap();
                    } else if olduniq.is_empty() {
                        Self::add(&dbus, atx, cur_state, name).await;
                    }
                });
            },
        );

        MediaService {
            dbus,
            noc_sub: Some(noc_sub),
            rx,
        }
    }

    async fn add(
        dbus: &gio::DBusConnection,
        atx: Arc<RefCell<bus::Publisher<HashMap<String, MediaPlayerState>>>>,
        cur_state: Arc<
            RefCell<(
                HashMap<String, MediaPlayerState>,
                HashMap<String, MediaSubscription>,
            )>,
        >,
        name: String,
    ) {
        if let (Ok(common), Ok(player)) = (
            gio::DBusProxy::new_future(
                dbus,
                gio::DBusProxyFlags::NONE,
                None,
                Some(&name),
                "/org/mpris/MediaPlayer2",
                "org.mpris.MediaPlayer2",
            )
            .await,
            gio::DBusProxy::new_future(
                dbus,
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
                    let cur_state = cur_state.clone();
                    let name = name.clone();
                    let atx = atx.clone();
                    common
                        .connect_local("g-properties-changed", true, move |args| {
                            let new_props = args[1]
                                .get::<glib::Variant>()
                                .unwrap()
                                .unwrap()
                                .get::<HashMap<String, glib::Variant>>()
                                .unwrap();
                            let cur_state = cur_state.clone();
                            let name = name.clone();
                            let atx = atx.clone();
                            glib::MainContext::default().spawn_local(async move {
                                let mut stref = cur_state.borrow_mut();
                                if let Some(ref mut obj) = stref.0.get_mut(&name) {
                                    obj.common_update(new_props);
                                }
                                atx.borrow_mut().send(stref.0.clone()).await.unwrap()
                            });
                            None
                        })
                        .ok()
                };
                let player_sub = {
                    let cur_state = cur_state.clone();
                    let name = name.clone();
                    let atx = atx.clone();
                    player
                        .connect_local("g-properties-changed", true, move |args| {
                            let new_props = args[1]
                                .get::<glib::Variant>()
                                .unwrap()
                                .unwrap()
                                .get::<HashMap<String, glib::Variant>>()
                                .unwrap();
                            let cur_state = cur_state.clone();
                            let name = name.clone();
                            let atx = atx.clone();
                            glib::MainContext::default().spawn_local(async move {
                                let mut stref = cur_state.borrow_mut();
                                if let Some(ref mut obj) = stref.0.get_mut(&name) {
                                    obj.player_update(new_props);
                                }
                                atx.borrow_mut().send(stref.0.clone()).await.unwrap()
                            });
                            None
                        })
                        .ok()
                };
                let mut stref = cur_state.borrow_mut();
                stref.0.insert(name.clone(), initial_state);
                stref.1.insert(
                    name,
                    MediaSubscription {
                        common,
                        common_sub,
                        player,
                        player_sub,
                    },
                );
                atx.borrow_mut().send(stref.0.clone()).await.unwrap();
            }
        } else {
            eprintln!("Failed to get proxies for MPRIS: {}", name);
        }
    }

    pub fn subscribe(&self) -> bus::Subscriber<MediaState> {
        self.rx.clone()
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
