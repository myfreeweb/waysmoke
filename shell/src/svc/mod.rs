pub mod media;
pub mod power;

pub struct Services {
    pub seat: wstk::wl_seat::WlSeat,
    pub toplevel_updates: wstk::bus::Subscriber<
        std::collections::HashMap<wstk::toplevels::ToplevelKey, wstk::toplevels::ToplevelState>,
    >,
    pub power: power::PowerService,
    pub media: media::MediaService,
}
