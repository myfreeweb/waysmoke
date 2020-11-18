use event_listener;
use futures::prelude::*;
use smithay_client_toolkit::{
    environment::GlobalHandler,
    reexports::client::{
        protocol::{wl_output, wl_registry},
        Attached, DispatchData, Proxy,
    },
};

pub use smithay_client_toolkit::reexports::protocols::wlr::unstable::foreign_toplevel::v1::client::{
    zwlr_foreign_toplevel_handle_v1 as toplevel_handle, zwlr_foreign_toplevel_manager_v1 as toplevel_manager,
};

use std::{
    cell::{Ref, RefCell},
    collections::HashMap,
    hash::{Hash, Hasher},
    rc::Rc,
};

#[derive(PartialEq, Eq, Clone)]
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

pub type ToplevelStates = HashMap<ToplevelKey, ToplevelState>;

pub struct ToplevelService {
    global: RefCell<Option<Attached<toplevel_manager::ZwlrForeignToplevelManagerV1>>>,
    notifier: Rc<event_listener::Event>,
    states: Rc<RefCell<ToplevelStates>>,
}

impl ToplevelService {
    pub fn new() -> ToplevelService {
        ToplevelService {
            global: RefCell::new(None),
            notifier: Rc::new(event_listener::Event::new()),
            states: Rc::new(RefCell::new(HashMap::new())),
        }
    }

    pub fn state(&self) -> Ref<'_, ToplevelStates> {
        self.states.borrow()
    }

    pub fn subscribe(&self) -> impl Future<Output = ()> {
        self.notifier.listen()
    }
}

pub struct ToplevelServiceRc(pub Rc<ToplevelService>);

impl GlobalHandler<toplevel_manager::ZwlrForeignToplevelManagerV1> for ToplevelServiceRc {
    fn created(&mut self, registry: Attached<wl_registry::WlRegistry>, id: u32, version: u32, _: DispatchData) {
        let main = registry.bind::<toplevel_manager::ZwlrForeignToplevelManagerV1>(version, id);
        let states = self.0.states.clone();
        let notifier = self.0.notifier.clone();
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
                let notifier = notifier.clone();
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
                    toplevel_handle::Event::OutputLeave { output } => topl.outputs.retain(|o| *o != output),
                    toplevel_handle::Event::State { state } => topl.state = state,
                    toplevel_handle::Event::Done => {
                        states
                            .borrow_mut()
                            .insert(ToplevelKey(topl.handle.clone()), topl.clone());
                        notifier.notify(usize::MAX);
                    }
                    toplevel_handle::Event::Closed => {
                        states.borrow_mut().remove(&ToplevelKey(topl.handle.clone()));
                        notifier.notify(usize::MAX);
                    }
                    toplevel_handle::Event::Parent { .. } => {}
                    x => panic!("Unknown toplevel event {:?}", x),
                });
            }
            x => panic!("Unknown toplevel manager event {:?}", x),
        });
        *self.0.global.borrow_mut() = Some((*main).clone())
    }

    fn get(&self) -> Option<Attached<toplevel_manager::ZwlrForeignToplevelManagerV1>> {
        self.0.global.borrow().clone()
    }
}

pub trait HasToplevelService {
    fn toplevel_service(&self) -> Rc<ToplevelService>;
}

impl HasToplevelService for Rc<ToplevelService> {
    fn toplevel_service(&self) -> Rc<ToplevelService> {
        self.clone()
    }
}

macro_rules! toplevel_handler {
    ($env:ident, $field:ident) => {
        impl HasToplevelService for $env {
            fn toplevel_service(&self) -> Rc<ToplevelService> {
                self.$field.0.clone()
            }
        }
    };
}
