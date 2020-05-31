// linicon doesn't have the XDG_DATA_DIRS fallback paths from the spec
const PATHS: &[&str] = &["/usr/local/share/icons", "/usr/share/icons"];

pub enum App {
    Known(gio::DesktopAppInfo),
    Unknown(String),
}

impl App {
    pub fn lookup(app_id: &str, title: Option<&str>) -> App {
        // TODO: gio::DesktopAppInfo::search for approximate matches
        if let Some(ai) = gio::DesktopAppInfo::new(&format!("{}.desktop", app_id)) {
            App::Known(ai)
        } else {
            App::Unknown(title.unwrap_or(app_id).to_owned())
        }
    }

    pub fn icon(&self) -> linicon::IconPath {
        // TODO: don't unwrap so much
        match self {
            App::Known(ai) => {
                use gio::AppInfoExt;
                use glib::object::Cast;
                let names = ai
                    .get_icon()
                    .unwrap()
                    .downcast::<gio::ThemedIcon>()
                    .unwrap()
                    .get_names();
                let name = names.iter().next().unwrap();
                linicon::lookup_icon_with_extra_paths("Adwaita", name, 64, 1, PATHS)
                    .unwrap()
                    .flat_map(|x| x)
                    .next()
                    .unwrap()
            }
            App::Unknown(_) => linicon::lookup_icon_with_extra_paths(
                "Adwaita",
                "application-x-executable",
                64,
                1,
                PATHS,
            )
            .unwrap()
            .flat_map(|x| x)
            .next()
            .unwrap(),
        }
    }
}
