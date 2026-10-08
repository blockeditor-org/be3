use std::collections::HashMap;
use std::path::{Path, PathBuf};

use beui::Image;

use super::Environment;

const FALLBACK_THEME: &str = "hicolor";
const DEFAULT_THEMES: [&str; 2] = ["Adwaita", "breeze"];
const EXTENSIONS: [&str; 2] = ["png", "svg"];
const MAX_INHERITED: usize = 16;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    Fixed,
    Scalable,
    Threshold,
}

#[derive(Clone, Debug)]
struct IconDir {
    roots: Vec<PathBuf>,
    size: u32,
    min: u32,
    max: u32,
    threshold: u32,
    kind: Kind,
}

impl IconDir {
    fn matches(&self, size: u32) -> bool {
        match self.kind {
            Kind::Fixed => self.size == size,
            Kind::Scalable => (self.min..=self.max).contains(&size),
            Kind::Threshold => {
                self.size.saturating_sub(self.threshold) <= size
                    && size <= self.size + self.threshold
            }
        }
    }

    fn distance(&self, size: u32) -> u32 {
        let (low, high) = match self.kind {
            Kind::Fixed => (self.size, self.size),
            Kind::Scalable => (self.min, self.max),
            Kind::Threshold => (
                self.size.saturating_sub(self.threshold),
                self.size + self.threshold,
            ),
        };
        match size {
            size if size < low => low - size,
            size if size > high => size - high,
            _ => 0,
        }
    }

    fn find(&self, name: &str) -> Option<PathBuf> {
        self.roots.iter().find_map(|root| {
            EXTENSIONS
                .iter()
                .map(|extension| root.join(format!("{name}.{extension}")))
                .find(|path| path.is_file())
        })
    }
}

#[derive(Clone, Debug, Default)]
pub struct IconThemes {
    themes: Vec<Vec<IconDir>>,
    pixmaps: Vec<PathBuf>,
}

impl IconThemes {
    pub fn new(environment: &Environment) -> Self {
        let mut bases = Vec::new();
        if let Some(home) = &environment.home {
            bases.push(home.join(".icons"));
        }
        bases.extend(environment.data_dirs.iter().map(|dir| dir.join("icons")));
        let chosen = environment
            .config_home()
            .as_deref()
            .and_then(gtk_theme)
            .into_iter()
            .chain(DEFAULT_THEMES.iter().map(|name| (*name).to_owned()))
            .find(|name| {
                bases
                    .iter()
                    .any(|base| base.join(name).join("index.theme").is_file())
            });
        let mut names: Vec<String> = Vec::new();
        let mut pending: Vec<String> = chosen.into_iter().collect();
        while let Some(name) = pending.pop() {
            if names.contains(&name) || name == FALLBACK_THEME || names.len() >= MAX_INHERITED {
                continue;
            }
            let Some(index) = read_index(&bases, &name) else {
                continue;
            };
            names.push(name);
            pending.extend(index.inherits.into_iter().rev());
        }
        names.push(FALLBACK_THEME.to_owned());
        let themes = names
            .iter()
            .filter_map(|name| Some(theme_dirs(&bases, name, read_index(&bases, name)?)))
            .collect();
        Self {
            themes,
            pixmaps: environment
                .data_dirs
                .iter()
                .map(|dir| dir.join("pixmaps"))
                .collect(),
        }
    }

    pub fn find(&self, name: &str, size: u32) -> Option<PathBuf> {
        if name.starts_with('/') {
            return Path::new(name).is_file().then(|| PathBuf::from(name));
        }
        for dirs in &self.themes {
            if let Some(found) = dirs
                .iter()
                .filter(|dir| dir.matches(size))
                .find_map(|dir| dir.find(name))
            {
                return Some(found);
            }
            let mut closest: Vec<&IconDir> = dirs.iter().collect();
            closest.sort_by_key(|dir| (dir.distance(size), dir.kind != Kind::Scalable));
            if let Some(found) = closest.into_iter().find_map(|dir| dir.find(name)) {
                return Some(found);
            }
        }
        self.pixmaps.iter().find_map(|dir| {
            EXTENSIONS
                .iter()
                .map(|extension| dir.join(format!("{name}.{extension}")))
                .find(|path| path.is_file())
        })
    }
}

struct Index {
    inherits: Vec<String>,
    groups: HashMap<String, HashMap<String, String>>,
    directories: Vec<String>,
}

fn read_index(bases: &[PathBuf], name: &str) -> Option<Index> {
    let text = bases
        .iter()
        .find_map(|base| std::fs::read_to_string(base.join(name).join("index.theme")).ok())?;
    let mut groups: HashMap<String, HashMap<String, String>> = HashMap::new();
    let mut current = None;
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if line.starts_with('[') && line.ends_with(']') {
            current = Some(line[1..line.len() - 1].to_owned());
            continue;
        }
        if let (Some(group), Some((key, value))) = (&current, line.split_once('=')) {
            groups
                .entry(group.clone())
                .or_default()
                .entry(key.trim().to_owned())
                .or_insert_with(|| value.trim().to_owned());
        }
    }
    let main = groups.get("Icon Theme").cloned().unwrap_or_default();
    let list = |key: &str| {
        main.get(key)
            .map(|value| {
                value
                    .split(',')
                    .map(str::trim)
                    .filter(|item| !item.is_empty())
                    .map(str::to_owned)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default()
    };
    let mut directories = list("Directories");
    directories.extend(list("ScaledDirectories"));
    Some(Index {
        inherits: list("Inherits"),
        groups,
        directories,
    })
}

fn theme_dirs(bases: &[PathBuf], name: &str, index: Index) -> Vec<IconDir> {
    let homes: Vec<PathBuf> = bases
        .iter()
        .map(|base| base.join(name))
        .filter(|home| home.is_dir())
        .collect();
    index
        .directories
        .iter()
        .filter_map(|directory| {
            let group = index.groups.get(directory)?;
            let number = |key: &str| group.get(key).and_then(|value| value.parse::<u32>().ok());
            let size = number("Size")?;
            let scale = number("Scale").unwrap_or(1).max(1);
            let roots: Vec<PathBuf> = homes
                .iter()
                .map(|home| home.join(directory))
                .filter(|root| root.is_dir())
                .collect();
            if roots.is_empty() {
                return None;
            }
            Some(IconDir {
                roots,
                size: size * scale,
                min: number("MinSize").unwrap_or(size) * scale,
                max: number("MaxSize").unwrap_or(size) * scale,
                threshold: number("Threshold").unwrap_or(2) * scale,
                kind: match group.get("Type").map(String::as_str) {
                    Some("Fixed") => Kind::Fixed,
                    Some("Scalable") => Kind::Scalable,
                    _ => Kind::Threshold,
                },
            })
        })
        .collect()
}

fn gtk_theme(config: &Path) -> Option<String> {
    ["gtk-4.0", "gtk-3.0"].iter().find_map(|version| {
        let text = std::fs::read_to_string(config.join(version).join("settings.ini")).ok()?;
        text.lines().find_map(|line| {
            let (key, value) = line.split_once('=')?;
            (key.trim() == "gtk-icon-theme-name")
                .then(|| value.trim().trim_matches('"').to_owned())
                .filter(|value| !value.is_empty())
        })
    })
}

pub fn load_icon(path: &Path, pixels: u32) -> Option<Image> {
    let data = std::fs::read(path).ok()?;
    match path.extension().and_then(|extension| extension.to_str()) {
        Some("svg") => render_svg(&data, pixels),
        _ => {
            let decoded =
                image::load_from_memory_with_format(&data, image::ImageFormat::Png).ok()?;
            let decoded = match decoded.width().max(decoded.height()) > pixels * 2 {
                true => decoded.resize(pixels, pixels, image::imageops::FilterType::Triangle),
                false => decoded,
            };
            let rgba = decoded.into_rgba8();
            let (width, height) = rgba.dimensions();
            (width > 0 && height > 0).then(|| Image::from_rgba(width, height, rgba.into_raw()))
        }
    }
}

fn render_svg(data: &[u8], pixels: u32) -> Option<Image> {
    let tree = resvg::usvg::Tree::from_data(data, &resvg::usvg::Options::default()).ok()?;
    let size = tree.size();
    let scale = pixels as f32 / size.width().max(size.height());
    let width = ((size.width() * scale).round() as u32).max(1);
    let height = ((size.height() * scale).round() as u32).max(1);
    let mut pixmap = resvg::tiny_skia::Pixmap::new(width, height)?;
    resvg::render(
        &tree,
        resvg::tiny_skia::Transform::from_scale(scale, scale),
        &mut pixmap.as_mut(),
    );
    let pixels = pixmap
        .pixels()
        .iter()
        .flat_map(|pixel| {
            let color = pixel.demultiply();
            [color.red(), color.green(), color.blue(), color.alpha()]
        })
        .collect();
    Some(Image::from_rgba(width, height, pixels))
}
