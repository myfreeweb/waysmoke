use crate::{dock::*, style, svc::power::*, util::apps};

pub struct PowerDocklet {
    pub icon: wstk::ImageHandle,
    pub evl: addeventlistener::State,
    pub rx: wstk::bus::Subscriber<svc::power::PowerState>,
}

impl PowerDocklet {
    pub fn new(rx: wstk::bus::Subscriber<svc::power::PowerState>) -> Self {
        PowerDocklet {
            icon: icons::icon_from_path(apps::icon("dialog-question")),
            evl: Default::default(),
            rx,
        }
    }
}

#[async_trait(?Send)]
impl Docklet for PowerDocklet {
    fn widget(&mut self, ctx: &DockCtx) -> Element<DockletMsg> {
        use iced_native::*;

        let img = icons::icon_widget(self.icon.clone());

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
        icons::ICON_SIZE + APP_PADDING * 2
    }

    fn update(&mut self, ctx: &DockCtx, msg: DockletMsg) {}

    async fn run(&mut self) {
        let st = self.rx.next().await.unwrap();
        self.icon = icons::icon_from_path(apps::icon(match st.total {
            Some(PowerDeviceState::Battery { ref icon_name, .. }) => {
                icon_name.trim_end_matches("-symbolic")
            }
            _ => "ac-adapter",
        }))
    }
}
