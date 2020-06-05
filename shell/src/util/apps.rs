use std::path::Path;

lazy_static::lazy_static! {
    static ref USERPATH: String =
        glib::get_user_data_dir().unwrap().join("icons").to_str().unwrap().to_owned();
    static ref PATHS: Vec<&'static str> = vec![
        // linicon doesn't have the XDG_DATA_DIRS fallback paths from the spec
        "/usr/local/share/icons", "/usr/share/icons",
        &*USERPATH,
    ];
}

#[derive(Debug, Eq)]
pub struct App {
    pub id: String,
    pub info: gio::DesktopAppInfo,
}

impl PartialEq for App {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl App {
    pub fn lookup(app_id: &str) -> Option<App> {
        // https://gitlab.gnome.org/GNOME/gnome-shell/-/blob/68745328df0b401ef08caec05e4d297d0a9e36b7/src/shell-app-system.c#L373-377
        if let Some(info) = gio::DesktopAppInfo::new(&format!("{}.desktop", app_id)).or_else(|| {
            gio::DesktopAppInfo::new(&format!(
                "{}.desktop",
                app_id.to_ascii_lowercase().replace(' ', "-")
            ))
        }) {
            Some(App {
                id: app_id.to_owned(),
                info,
            })
        } else {
            None
        }
    }

    pub fn icon(&self) -> Option<linicon::IconPath> {
        use gio::AppInfoExt;
        use glib::object::Cast;
        use linicon::IconType::*;
        let names = self
            .info
            .get_icon()?
            .downcast::<gio::ThemedIcon>()
            .unwrap()
            .get_names();
        let name = names.iter().next()?;
        linicon::lookup_icon_with_extra_paths("Adwaita", name, 64, 1, &PATHS[..])
            .unwrap()
            .flat_map(|x| x)
            .next()
            .or_else(|| check_icon(format!("/usr/local/share/pixmaps/{}.svg", name), SVG))
            .or_else(|| check_icon(format!("/usr/local/share/pixmaps/{}.png", name), PNG))
            .or_else(|| check_icon(format!("/usr/share/pixmaps/{}.svg", name), SVG))
            .or_else(|| check_icon(format!("/usr/share/pixmaps/{}.png", name), PNG))
    }
}

fn check_icon(p: String, icon_type: linicon::IconType) -> Option<linicon::IconPath> {
    let path = Path::new(&p);
    if path.exists() {
        Some(linicon::IconPath {
            path: path.to_owned(),
            theme: "<pixmaps>".to_owned(),
            icon_type,
        })
    } else {
        None
    }
}

pub fn unknown_icon() -> linicon::IconPath {
    linicon::lookup_icon_with_extra_paths("Adwaita", "application-x-executable", 64, 1, &PATHS[..])
        .unwrap()
        .flat_map(|x| x)
        .next()
        .unwrap()
}
