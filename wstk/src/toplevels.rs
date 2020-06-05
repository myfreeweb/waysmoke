use crate::event_loop::wayland_event_chan;
use futures::{channel::mpsc, prelude::*};
use smithay_client_toolkit::{
    environment::GlobalHandler,
    reexports::client::{
        protocol::{wl_output, wl_registry},
        Attached, DispatchData, Interface, MessageGroup, Proxy, ProxyMap,
    },
};

pub use smithay_client_toolkit::reexports::protocols::wlr::unstable::foreign_toplevel::v1::client::{
    zwlr_foreign_toplevel_handle_v1 as toplevel_handle,
    zwlr_foreign_toplevel_manager_v1 as toplevel_manager,
};

use std::{
    cell::RefCell,
    collections::HashMap,
    hash::{Hash, Hasher},
    rc::Rc,
    sync::Arc,
};

#[derive(PartialEq, Eq)]
pub struct ToplevelKey(toplevel_handle::ZwlrForeignToplevelHandleV1);

impl Hash for ToplevelKey {
    fn hash<H: Hasher>(&self, state: &mut H) {
        Proxy::from(self.0.clone()).c_ptr().hash(state);
    }
}

#[derive(Clone)]
pub struct ToplevelState {
    pub handle: toplevel_handle::ZwlrForeignToplevelHandleV1,
    pub title: String,
    pub app_id: String,
    pub gtk_app_id: Option<String>,
    pub outputs: Vec<wl_output::WlOutput>,
    pub state: Vec<u8>,
}

impl ToplevelState {
    pub fn matches_id(&self, id: &str) -> bool {
        id == self.app_id || self.gtk_app_id.as_ref().map(|x| id == x).unwrap_or(false)
    }
}

pub type ToplevelStates = Rc<RefCell<HashMap<ToplevelKey, ToplevelState>>>;

pub struct ToplevelHandler {
    global: Option<Attached<toplevel_manager::ZwlrForeignToplevelManagerV1>>,
    states: ToplevelStates,
    channels: Rc<RefCell<Vec<mpsc::UnboundedSender<()>>>>,
}

impl ToplevelHandler {
    pub fn new() -> ToplevelHandler {
        ToplevelHandler {
            global: None,
            states: Rc::new(RefCell::new(HashMap::new())),
            channels: Rc::new(RefCell::new(Vec::new())),
        }
    }
}

impl GlobalHandler<toplevel_manager::ZwlrForeignToplevelManagerV1> for ToplevelHandler {
    fn created(
        &mut self,
        registry: Attached<wl_registry::WlRegistry>,
        id: u32,
        version: u32,
        _: DispatchData,
    ) {
        let main = registry.bind::<toplevel_manager::ZwlrForeignToplevelManagerV1>(version, id);
        let states = self.states.clone();
        let channels = self.channels.clone();
        main.quick_assign(move |_, event, _| match event {
            toplevel_manager::Event::Toplevel { toplevel } => {
                let mut topl = ToplevelState {
                    handle: toplevel.detach(),
                    title: "".to_owned(),
                    app_id: "".to_owned(),
                    gtk_app_id: None,
                    outputs: Vec::new(),
                    state: Vec::new(),
                };
                let states = states.clone();
                let channels = channels.clone();
                toplevel.quick_assign(move |_, event, _| match event {
                    toplevel_handle::Event::Title { title } => topl.title = title,
                    toplevel_handle::Event::AppId { app_id } => {
                        // Wayfire with option workarounds/app_id_mode == "full" adds gtk-shell id after a space
                        let mut words = app_id.split(' ');
                        topl.app_id = words.next().unwrap_or("").to_owned();
                        topl.gtk_app_id = words.next().map(|x| x.to_owned());
                        if words.next().is_some() {
                            eprintln!("WARN: app_id with more than one space: '{}'", app_id);
                        }
                    }
                    toplevel_handle::Event::OutputEnter { output } => topl.outputs.push(output),
                    toplevel_handle::Event::OutputLeave { output } => {
                        topl.outputs.retain(|o| *o != output)
                    }
                    toplevel_handle::Event::State { state } => topl.state = state,
                    toplevel_handle::Event::Done => {
                        states
                            .borrow_mut()
                            .insert(ToplevelKey(topl.handle.clone()), topl.clone());
                        let channels = channels.clone();
                        glib::MainContext::default().spawn_local(async move {
                            let chans = channels.borrow_mut();
                            for mut chan in chans.iter() {
                                chan.send(()).await.unwrap()
                            }
                        });
                    }
                    toplevel_handle::Event::Closed => {
                        states
                            .borrow_mut()
                            .remove(&ToplevelKey(topl.handle.clone()));
                        let channels = channels.clone();
                        glib::MainContext::default().spawn_local(async move {
                            let chans = channels.borrow_mut();
                            for mut chan in chans.iter() {
                                chan.send(()).await.unwrap()
                            }
                        });
                    }
                    _ => panic!(""),
                });
            }
            _ => panic!(""),
        });
        self.global = Some((*main).clone())
    }

    fn get(&self) -> Option<Attached<toplevel_manager::ZwlrForeignToplevelManagerV1>> {
        self.global.clone()
    }
}

pub trait HasToplevelManager {
    fn toplevel_updates(&mut self) -> mpsc::UnboundedReceiver<()>;
    fn toplevels(&self) -> Rc<RefCell<HashMap<ToplevelKey, ToplevelState>>>;
}

impl HasToplevelManager for ToplevelHandler {
    fn toplevel_updates(&mut self) -> mpsc::UnboundedReceiver<()> {
        let (tx, rx) = mpsc::unbounded();
        self.channels.borrow_mut().push(tx);
        rx
    }

    fn toplevels(&self) -> Rc<RefCell<HashMap<ToplevelKey, ToplevelState>>> {
        self.states.clone()
    }
}

macro_rules! toplevel_handler {
    ($env:ident, $field:ident) => {
        impl HasToplevelManager for $env {
            fn toplevel_updates(&mut self) -> futures::channel::mpsc::UnboundedReceiver<()> {
                self.$field.toplevel_updates()
            }
            fn toplevels(
                &self,
            ) -> std::rc::Rc<
                std::cell::RefCell<std::collections::HashMap<ToplevelKey, ToplevelState>>,
            > {
                self.$field.toplevels()
            }
        }
    };
}
