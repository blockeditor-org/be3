use stable_hash::HashSet;
use std::path::{Path, PathBuf};

const DEFAULT_SIZE: u32 = 24;

#[derive(Clone, Debug, PartialEq)]
pub struct Theme {
    name: String,
    size: u32,
    paths: Vec<PathBuf>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Environment {
    pub theme: Option<String>,
    pub size: Option<String>,
    pub home: Option<String>,
    pub xcursor_path: Option<String>,
    pub xdg_data_home: Option<String>,
    pub xdg_data_dirs: Option<String>,
}

impl Environment {
    pub fn read() -> Self {
        let var = |name: &str| std::env::var(name).ok().filter(|value| !value.is_empty());
        Self {
            theme: var("XCURSOR_THEME"),
            size: var("XCURSOR_SIZE"),
            home: var("HOME"),
            xcursor_path: var("XCURSOR_PATH"),
            xdg_data_home: var("XDG_DATA_HOME"),
            xdg_data_dirs: var("XDG_DATA_DIRS"),
        }
    }

    pub fn search_paths(&self) -> Vec<PathBuf> {
        let home = self.home.as_deref().map(Path::new);
        let expand = |entry: &str| -> Option<PathBuf> {
            if entry.is_empty() {
                return None;
            }
            match entry.strip_prefix("~/") {
                Some(rest) => Some(home?.join(rest)),
                None if entry == "~" => home.map(Path::to_path_buf),
                None => Some(PathBuf::from(entry)),
            }
        };
        if let Some(listed) = &self.xcursor_path {
            return listed.split(':').filter_map(expand).collect();
        }
        let mut paths = Vec::new();
        match &self.xdg_data_home {
            Some(data) => paths.extend(expand(data).map(|data| data.join("icons"))),
            None => paths.extend(home.map(|home| home.join(".local/share/icons"))),
        }
        paths.extend(home.map(|home| home.join(".icons")));
        match &self.xdg_data_dirs {
            Some(dirs) => paths.extend(
                dirs.split(':')
                    .filter_map(expand)
                    .map(|dir| dir.join("icons")),
            ),
            None => paths.extend(["/usr/local/share/icons", "/usr/share/icons"].map(PathBuf::from)),
        }
        paths.push(PathBuf::from("/usr/share/pixmaps"));
        paths.push(PathBuf::from("/usr/share/cursors/xorg-x11"));
        paths
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Image {
    pub nominal: u32,
    pub width: u32,
    pub height: u32,
    pub hotspot: (u32, u32),
    pub rgba: Vec<u8>,
}

impl Theme {
    pub fn from_env() -> Self {
        Self::of(&Environment::read())
    }

    pub fn of(environment: &Environment) -> Self {
        let size = environment
            .size
            .as_deref()
            .and_then(|size| size.trim().parse().ok())
            .filter(|size| *size > 0)
            .unwrap_or(DEFAULT_SIZE);
        Self::new(
            environment.theme.as_deref().unwrap_or("default"),
            size,
            environment.search_paths(),
        )
    }

    pub fn new(name: &str, size: u32, paths: Vec<PathBuf>) -> Self {
        Self {
            name: name.to_owned(),
            size,
            paths,
        }
    }

    pub fn size(&self) -> u32 {
        self.size
    }

    pub fn find(&self, cursor: &str) -> Option<PathBuf> {
        if cursor.is_empty() || cursor.contains('/') {
            return None;
        }
        let mut walked = HashSet::default();
        self.find_in(&self.name, cursor, &mut walked)
            .or_else(|| self.find_in("default", cursor, &mut walked))
    }

    fn find_in(&self, theme: &str, cursor: &str, walked: &mut HashSet<String>) -> Option<PathBuf> {
        if !walked.insert(theme.to_owned()) {
            return None;
        }
        let dirs: Vec<PathBuf> = self
            .paths
            .iter()
            .map(|path| path.join(theme))
            .filter(|dir| dir.is_dir())
            .collect();
        if let Some(found) = dirs
            .iter()
            .map(|dir| dir.join("cursors").join(cursor))
            .find(|path| path.is_file())
        {
            return Some(found);
        }
        dirs.iter()
            .flat_map(|dir| inherits(&dir.join("index.theme")))
            .find_map(|parent| self.find_in(&parent, cursor, walked))
    }

    pub fn load(&self, names: &[&str], pixels: u32) -> Option<Image> {
        names.iter().find_map(|name| {
            let path = self.find(name)?;
            let bytes = std::fs::read(&path).ok()?;
            nearest(xcursor::parser::parse_xcursor(&bytes)?, pixels)
        })
    }
}

fn nearest(images: Vec<xcursor::parser::Image>, pixels: u32) -> Option<Image> {
    let nominal = images
        .iter()
        .map(|image| image.size)
        .min_by_key(|size| (size.abs_diff(pixels), std::cmp::Reverse(*size)))?;
    let image = images.into_iter().find(|image| image.size == nominal)?;
    let expected = image.width as usize * image.height as usize * 4;
    if image.width == 0 || image.height == 0 || image.pixels_rgba.len() != expected {
        return None;
    }
    Some(Image {
        nominal,
        width: image.width,
        height: image.height,
        hotspot: (
            image.xhot.min(image.width - 1),
            image.yhot.min(image.height - 1),
        ),
        rgba: image
            .pixels_rgba
            .as_chunks::<4>()
            .0
            .iter()
            .flat_map(|bgra| [bgra[2], bgra[1], bgra[0], bgra[3]])
            .collect(),
    })
}

fn inherits(index: &Path) -> Vec<String> {
    let Ok(text) = std::fs::read_to_string(index) else {
        return Vec::new();
    };
    let mut section = "";
    for line in text.lines().map(str::trim) {
        if let Some(name) = line
            .strip_prefix('[')
            .and_then(|line| line.strip_suffix(']'))
        {
            section = name;
            continue;
        }
        if !section.is_empty() && section != "Icon Theme" {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        if key.trim() == "Inherits" {
            return value
                .split([',', ';'])
                .map(str::trim)
                .filter(|name| !name.is_empty())
                .map(str::to_owned)
                .collect();
        }
    }
    Vec::new()
}

#[cfg(test)]
pub(crate) mod tests;
