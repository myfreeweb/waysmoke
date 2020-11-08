use futures::prelude::*;
use gio::prelude::*;
use std::{cell::RefCell, collections::HashMap, sync::Arc};
use wstk::bus;

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
        match dev.get_cached_property("Type")?.get::<u32>().unwrap() {
            1 => Some(PowerDeviceState::Line {
                online: dev.get_cached_property("Online").unwrap().get().unwrap(),
            }),
            2 => Some(PowerDeviceState::Battery {
                icon_name: dev.get_cached_property("IconName").unwrap().get().unwrap(),
                percentage: dev
                    .get_cached_property("Percentage")
                    .unwrap()
                    .get()
                    .unwrap(),
                energy: dev.get_cached_property("Energy").unwrap().get().unwrap(),
                energy_empty: dev
                    .get_cached_property("EnergyEmpty")
                    .unwrap()
                    .get()
                    .unwrap(),
                energy_full: dev
                    .get_cached_property("EnergyFull")
                    .unwrap()
                    .get()
                    .unwrap(),
                energy_rate: dev
                    .get_cached_property("EnergyRate")
                    .unwrap()
                    .get()
                    .unwrap(),
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

#[derive(Clone)]
pub struct PowerService {
    display_device: gio::DBusProxy,
}

impl PowerService {
    pub async fn new(dbus: &gio::DBusConnection) -> (PowerService, bus::Subscriber<PowerState>) {
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

        let cur_state = Arc::new(RefCell::new(PowerState {
            total: PowerDeviceState::query(&display_device),
        }));

        let (mut tx, rx) = bus::bounded(1);
        tx.send(cur_state.borrow().clone()).await.unwrap();
        let atx = Arc::new(RefCell::new(tx));
        display_device
            .connect_local("g-properties-changed", true, move |args| {
                let new_props = args[1]
                    .get::<glib::Variant>()
                    .unwrap()
                    .unwrap()
                    .get::<HashMap<String, glib::Variant>>()
                    .unwrap();
                let mut stref = cur_state.borrow_mut();
                if let Some(ref mut total) = stref.total {
                    total.update(new_props);
                }
                let nst = stref.clone();
                let atx = atx.clone();
                glib::MainContext::default()
                    .spawn_local(async move { atx.borrow_mut().send(nst).await.unwrap() });
                None
            })
            .unwrap();

        (PowerService { display_device }, rx)
    }
}
