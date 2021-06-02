use futures::prelude::*;
use gio::prelude::*;
use std::{
    cell::{Ref, RefCell},
    collections::HashMap,
    rc::Rc,
};
use wstk::event_listener;

#[derive(Debug, Clone)]
pub enum PowerDeviceState {
    Battery {
        icon_name: String,
        percentage: f64,
        energy: f64,       // Wh
        energy_empty: f64, // Wh
        energy_full: f64,  // Wh
        energy_rate: f64,  // W
    },
    Line {
        online: bool,
    },
}

impl PowerDeviceState {
    fn query(dev: &gio::DBusProxy) -> Option<PowerDeviceState> {
        match dev.cached_property("Type")?.get::<u32>().unwrap() {
            1 => Some(PowerDeviceState::Line {
                online: dev.cached_property("Online").unwrap().get().unwrap(),
            }),
            2 => Some(PowerDeviceState::Battery {
                icon_name: dev.cached_property("IconName").unwrap().get().unwrap(),
                percentage: dev.cached_property("Percentage").unwrap().get().unwrap(),
                energy: dev.cached_property("Energy").unwrap().get().unwrap(),
                energy_empty: dev.cached_property("EnergyEmpty").unwrap().get().unwrap(),
                energy_full: dev.cached_property("EnergyFull").unwrap().get().unwrap(),
                energy_rate: dev.cached_property("EnergyRate").unwrap().get().unwrap(),
            }),
            _ => None, // TODO more
        }
    }

    fn update(&mut self, new_props: HashMap<String, glib::Variant>) {
        match self {
            PowerDeviceState::Battery {
                ref mut icon_name,
                ref mut percentage,
                ref mut energy,
                ref mut energy_empty,
                ref mut energy_full,
                ref mut energy_rate,
            } => {
                if let Some(e) = new_props.get("IconName").and_then(|e| e.get()) {
                    *icon_name = e;
                }
                if let Some(e) = new_props.get("Percentage").and_then(|e| e.get()) {
                    *percentage = e;
                }
                if let Some(e) = new_props.get("Energy").and_then(|e| e.get()) {
                    *energy = e;
                }
                if let Some(e) = new_props.get("EnergyEmpty").and_then(|e| e.get()) {
                    *energy_empty = e;
                }
                if let Some(e) = new_props.get("EnergyFull").and_then(|e| e.get()) {
                    *energy_full = e;
                }
                if let Some(e) = new_props.get("EnergyRate").and_then(|e| e.get()) {
                    *energy_rate = e;
                }
            }
            PowerDeviceState::Line { ref mut online } => {
                if let Some(o) = new_props.get("Online").and_then(|e| e.get()) {
                    *online = o;
                }
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct PowerState {
    pub total: Option<PowerDeviceState>,
    // TODO: all the devices
}

pub struct PowerService {
    // display_device: gio::DBusProxy,
    notifier: Rc<event_listener::Event>,
    state: Rc<RefCell<PowerState>>,
}

impl PowerService {
    pub async fn new(dbus: &gio::DBusConnection) -> PowerService {
        let notifier = Rc::new(event_listener::Event::new());

        let display_device = gio::DBusProxy::new_future(
            dbus,
            gio::DBusProxyFlags::NONE,
            None,
            Some("org.freedesktop.UPower"),
            "/org/freedesktop/UPower/devices/DisplayDevice",
            "org.freedesktop.UPower.Device",
        )
        .await
        .unwrap();

        let state = Rc::new(RefCell::new(PowerState {
            total: PowerDeviceState::query(&display_device),
        }));

        {
            let state = state.clone();
            let notifier = notifier.clone();
            display_device
                .connect_local("g-properties-changed", true, move |args| {
                    let new_props = args[1]
                        .get::<glib::Variant>()
                        .unwrap()
                        .get::<HashMap<String, glib::Variant>>()
                        .unwrap();
                    if let Some(ref mut total) = state.borrow_mut().total {
                        total.update(new_props);
                    }
                    notifier.notify(usize::MAX);
                    None
                })
                .unwrap();
        }

        PowerService {
            // display_device,
            notifier,
            state,
        }
    }

    pub fn state(&self) -> Ref<'_, PowerState> {
        self.state.borrow()
    }

    pub fn subscribe(&self) -> impl Future<Output = ()> {
        self.notifier.listen()
    }
}
