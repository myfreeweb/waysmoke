use raw_window_handle::{unix::WaylandHandle, HasRawWindowHandle, RawWindowHandle};
use smithay_client_toolkit::reexports::client::{
    protocol::{wl_display, wl_surface},
    Proxy,
};

pub struct ToRWH(
    pub Proxy<wl_surface::WlSurface>,
    pub Proxy<wl_display::WlDisplay>,
);

unsafe impl HasRawWindowHandle for ToRWH {
    fn raw_window_handle(&self) -> RawWindowHandle {
        RawWindowHandle::Wayland(WaylandHandle {
            surface: self.0.c_ptr() as *mut _,
            display: self.1.c_ptr() as *mut _,
            ..WaylandHandle::empty()
        })
    }
}
