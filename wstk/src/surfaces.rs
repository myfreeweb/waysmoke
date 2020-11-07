use smithay_client_toolkit::{default_environment, new_default_environment};
pub use smithay_client_toolkit::{
    environment::{Environment, SimpleGlobal},
    get_surface_scale_factor,
    reexports::{
        client::{
            protocol::{
                wl_compositor, wl_pointer, wl_region, wl_seat, wl_shm, wl_surface, wl_touch,
            },
            Attached, ConnectError, Display, EventQueue, Interface, Main, Proxy,
        },
        protocols::wlr::unstable::foreign_toplevel::v1::client::{
            zwlr_foreign_toplevel_handle_v1 as toplevel_handle,
            zwlr_foreign_toplevel_manager_v1 as toplevel_manager,
        },
        protocols::wlr::unstable::layer_shell::v1::client::{
            zwlr_layer_shell_v1 as layer_shell, zwlr_layer_surface_v1 as layer_surface,
        },
    },
    seat::{pointer, with_seat_data},
};

use futures::channel::mpsc;
pub use futures::prelude::*;

use crate::{handle::*, toplevels::*};

default_environment!(Env,
    fields = [
        layer_shell: SimpleGlobal<layer_shell::ZwlrLayerShellV1>,
        toplevel_manager: ToplevelHandler,
    ],
    singles = [
        layer_shell::ZwlrLayerShellV1 => layer_shell,
        toplevel_manager::ZwlrForeignToplevelManagerV1 => toplevel_manager,
    ],
);
toplevel_handler!(Env, toplevel_manager);

pub fn make_env() -> Result<(Environment<Env>, Display, EventQueue), ConnectError> {
    new_default_environment!(
        Env,
        fields = [
            layer_shell: SimpleGlobal::new(),
            toplevel_manager: ToplevelHandler::new(),
        ]
    )
}

static mut SCALE_CHANNELS: Vec<(wl_surface::WlSurface, mpsc::UnboundedSender<i32>)> = Vec::new();

pub trait DesktopSurface {
    fn setup_lsh(&self, layer_surface: &Main<layer_surface::ZwlrLayerSurfaceV1>);
}

pub struct DesktopInstance {
    pub env: Environment<Env>,
    pub display: Display,
    pub theme_mgr: pointer::ThemeManager,
    pub wl_surface: Attached<wl_surface::WlSurface>,
    pub layer_surface: Main<layer_surface::ZwlrLayerSurfaceV1>,
    pub scale_rx: mpsc::UnboundedReceiver<i32>,
}

impl DesktopInstance {
    pub fn new(
        surface: &dyn DesktopSurface,
        env: Environment<Env>,
        display: Display,
        _queue: &EventQueue,
    ) -> DesktopInstance {
        let theme_mgr = pointer::ThemeManager::init(
            pointer::ThemeSpec::System, // XCURSOR_THEME XCURSOR_SIZE env vars
            env.require_global::<wl_compositor::WlCompositor>(),
            env.require_global::<wl_shm::WlShm>(),
        );
        let layer_shell = env.require_global::<layer_shell::ZwlrLayerShellV1>();

        let (scale_tx, scale_rx) = mpsc::unbounded();
        let wl_surface: Attached<wl_surface::WlSurface> =
            env.create_surface_with_scale_callback(|scale, wlsurf, _dd| unsafe {
                SCALE_CHANNELS
                    .iter()
                    .find(|(surf, _)| *surf == wlsurf)
                    .unwrap()
                    .1
                    .unbounded_send(scale)
                    .unwrap();
            });
        unsafe {
            SCALE_CHANNELS.push((wl_surface.detach(), scale_tx));
        }

        let layer_surface = layer_shell.get_layer_surface(
            &wl_surface,
            None,
            layer_shell::Layer::Top,
            "Waysmoke Surface".to_owned(),
        );
        surface.setup_lsh(&layer_surface);

        DesktopInstance {
            env,
            display,
            theme_mgr,
            wl_surface,
            layer_surface,
            scale_rx,
        }
    }

    pub fn raw_handle(&self) -> ToRWH {
        ToRWH((*self.wl_surface.as_ref()).clone(), (*self.display).clone())
    }

    pub fn flush(&self) {
        self.display.flush().unwrap();
    }

    pub fn create_region(&self) -> Main<wl_region::WlRegion> {
        self.env
            .require_global::<wl_compositor::WlCompositor>()
            .create_region()
    }

    pub fn set_input_region(&self, region: Main<wl_region::WlRegion>) {
        self.wl_surface.set_input_region(Some(&region.detach()));
    }

    pub fn clear_input_region(&self) {
        self.wl_surface.set_input_region(None);
    }
}

impl Drop for DesktopInstance {
    fn drop(&mut self) {
        self.layer_surface.destroy();
        self.wl_surface.destroy();
    }
}
