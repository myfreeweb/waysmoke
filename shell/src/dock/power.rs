use crate::{dock::*, style, svc::power::*, util::apps};

pub struct PowerDocklet {
    icon: wstk::ImageHandle,
    evl: addeventlistener::State,
    svc: &'static svc::power::PowerService,
}

impl PowerDocklet {
    pub fn new(services: &'static svc::Services) -> Self {
        PowerDocklet {
            icon: Self::the_icon(&services.power.state()),
            evl: Default::default(),
            svc: &services.power,
        }
    }

    fn the_icon(st: &svc::power::PowerState) -> wstk::ImageHandle {
        icons::icon_from_path(apps::icon(match st.total {
            Some(PowerDeviceState::Battery { ref icon_name, .. }) => {
                icon_name.trim_end_matches("-symbolic")
            }
            _ => "ac-adapter",
        }))
    }
}

#[async_trait(?Send)]
impl Docklet for PowerDocklet {
    fn widget(&mut self) -> Element<DockletMsg> {
        use iced_native::*;

        let img = icons::icon_widget(self.icon.clone(), ICON_SIZE);

        let listener =
            AddEventListener::new(&mut self.evl, img).on_pointer_enter(DockletMsg::Hover);

        Container::new(listener)
            .center_x()
            .center_y()
            .padding(APP_PADDING)
            .style(style::Dock(style::DARK_COLOR))
            .into()
    }

    fn width(&self) -> u16 {
        ICON_SIZE + APP_PADDING * 2
    }

    fn retained_icon(&self) -> Option<wstk::ImageHandle> {
        Some(self.icon.clone())
    }

    fn update(&mut self, _msg: DockletMsg) {}

    async fn run(&mut self) {
        self.svc.subscribe().await;
        let st = self.svc.state();
        self.icon = Self::the_icon(&st);
    }
}
