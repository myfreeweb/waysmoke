pub mod media;
pub mod power;

pub struct Services {
    pub seat: wstk::wl_seat::WlSeat,
    pub toplevels: std::rc::Rc<wstk::toplevels::ToplevelService>,
    pub power: power::PowerService,
    pub media: media::MediaService,
}
