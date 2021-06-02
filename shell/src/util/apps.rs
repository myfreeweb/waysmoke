use crate::dock;
use gio::prelude::*;
use std::path::Path;

lazy_static::lazy_static! {
    static ref USERPATH: String =
        glib::user_data_dir().join("icons").to_str().unwrap().to_owned();
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
        if let Some(info) = gio::DesktopAppInfo::new(&format!("{}.desktop", app_id))
            .or_else(|| gio::DesktopAppInfo::new(&format!("{}.desktop", app_id.to_ascii_lowercase().replace(' ', "-"))))
        {
            Some(App {
                id: app_id.to_owned(),
                info,
            })
        } else {
            None
        }
    }

    pub fn icon(&self) -> Option<linicon::IconPath> {
        let icon = self.info.icon()?;
        if let Some(ticon) = icon.downcast_ref::<gio::ThemedIcon>() {
            return themed_icon(ticon);
        }
        if let Some(ficon) = icon.downcast_ref::<gio::FileIcon>() {
            let path: std::path::PathBuf = ficon.file().path()?;
            let icon_type = if tree_magic_mini::match_filepath("image/svg+xml", &path) {
                linicon::IconType::SVG
            } else if tree_magic_mini::match_filepath("image/png", &path) {
                linicon::IconType::PNG
            } else {
                eprintln!(
                    "Icon '{:?}' has unsupported type {:?}",
                    &path,
                    tree_magic_mini::from_filepath(&path)
                );
                return None;
            };
            return Some(linicon::IconPath {
                path,
                theme: "hicolor".to_string(),
                icon_type,
                min_size: 0,
                max_size: 420,
                scale: 1,
            });
        }
        None
    }
}

fn themed_icon(icon: &gio::ThemedIcon) -> Option<linicon::IconPath> {
    use linicon::IconType::*;
    let names = icon.names();
    let name = names.iter().next()?;
    // TODO: get current scale from caller instead of assuming 2
    icons_iter(name, dock::ICON_SIZE, 2)
        .chain(icons_iter(name, dock::ICON_SIZE * 2, 1))
        .chain(icons_iter(name, dock::ICON_SIZE, 1))
        .chain(icons_iter(name, 512, 1))
        .chain(icons_iter(name, 256, 1))
        .chain(icons_iter(name, 128, 1))
        .chain(icons_iter(name, 32, 1))
        .next()
        .or_else(|| check_icon(format!("/usr/local/share/pixmaps/{}.svg", name), SVG))
        .or_else(|| check_icon(format!("/usr/local/share/pixmaps/{}.png", name), PNG))
        .or_else(|| check_icon(format!("/usr/share/pixmaps/{}.svg", name), SVG))
        .or_else(|| check_icon(format!("/usr/share/pixmaps/{}.png", name), PNG))
}

fn icons_iter(name: &str, size: u16, scale: u16) -> impl Iterator<Item = linicon::IconPath> {
    linicon::lookup_icon(name)
        .from_theme("Adwaita")
        .with_search_paths(&PATHS[..])
        .unwrap()
        .with_size(size)
        .with_scale(scale)
        .flat_map(|x| x)
}

fn check_icon(p: String, icon_type: linicon::IconType) -> Option<linicon::IconPath> {
    let path = Path::new(&p);
    if path.exists() {
        Some(linicon::IconPath {
            path: path.to_owned(),
            theme: "<pixmaps>".to_owned(),
            icon_type,
            min_size: 0,
            max_size: 420,
            scale: 1,
        })
    } else {
        None
    }
}

pub fn icon_opt(name: &str, size: u16) -> Option<linicon::IconPath> {
    linicon::lookup_icon(name)
        .from_theme("Adwaita")
        .with_search_paths(&PATHS[..])
        .ok()?
        .with_size(size)
        .with_scale(1)
        .flat_map(|x| x)
        .next()
}

pub fn icon(name: &str) -> linicon::IconPath {
    icon_opt(name, dock::ICON_SIZE * 2).unwrap_or_else(|| {
        icon_opt(name, dock::ICON_SIZE)
            .unwrap_or_else(|| icon_opt("application-x-executable", dock::ICON_SIZE).unwrap())
    })
}
