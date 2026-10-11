use std::path::{Path, PathBuf};

use be_wayland::programs::{Environment, IconThemes, load_icon};
use block_plugin_api::{HostImage, TrayStatus};

use super::model::{Properties, pixmap_image, shrink};

pub(super) const ICON_PIXELS: u32 = 48;
const EXTENSIONS: [&str; 2] = ["png", "svg"];
const MAX_THEME_DEPTH: usize = 4;
const MAX_THEME_ENTRIES: usize = 4096;

pub(super) struct Icons {
    themes: IconThemes,
}

impl Icons {
    pub(super) fn new() -> Self {
        Self {
            themes: IconThemes::new(&Environment::from_env()),
        }
    }

    pub(super) fn icon(&self, properties: &Properties) -> Option<HostImage> {
        let attention = properties.status == TrayStatus::NeedsAttention;
        let wanted = attention
            .then_some((
                &properties.attention_icon_name,
                &properties.attention_pixmaps,
            ))
            .into_iter()
            .chain([(&properties.icon_name, &properties.icon_pixmaps)]);
        for (name, pixmaps) in wanted {
            if let Some(icon) = self.named(name, &properties.theme_path) {
                return Some(icon);
            }
            if let Some(icon) = pixmap_image(pixmaps, ICON_PIXELS) {
                return Some(icon);
            }
        }
        None
    }

    fn named(&self, name: &str, theme_path: &str) -> Option<HostImage> {
        let name = name.trim();
        if name.is_empty() || name.contains("..") {
            return None;
        }
        let path = match name.starts_with('/') {
            true => PathBuf::from(name),
            false => (!theme_path.is_empty())
                .then(|| in_theme_path(Path::new(theme_path), name))
                .flatten()
                .or_else(|| self.themes.find(name, ICON_PIXELS))?,
        };
        let image = load_icon(&path, ICON_PIXELS)?;
        Some(shrink(
            image.width(),
            image.height(),
            image.pixels().to_vec(),
            ICON_PIXELS,
        ))
    }
}

fn in_theme_path(root: &Path, name: &str) -> Option<PathBuf> {
    let mut pending = vec![(root.to_path_buf(), 0)];
    let mut seen = 0;
    let mut found: Vec<PathBuf> = Vec::new();
    while let Some((dir, depth)) = pending.pop() {
        for extension in EXTENSIONS {
            let path = dir.join(format!("{name}.{extension}"));
            if path.is_file() {
                found.push(path);
            }
        }
        if depth >= MAX_THEME_DEPTH {
            continue;
        }
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            seen += 1;
            if seen > MAX_THEME_ENTRIES {
                break;
            }
            if entry.file_type().is_ok_and(|kind| kind.is_dir()) {
                pending.push((entry.path(), depth + 1));
            }
        }
    }
    found.sort_by_key(|path| (size_from_path(path).abs_diff(ICON_PIXELS), path.clone()));
    found.into_iter().next()
}

fn size_from_path(path: &Path) -> u32 {
    path.components()
        .filter_map(|component| component.as_os_str().to_str())
        .find_map(|component| {
            let (width, _) = component.split_once('x')?;
            width.parse::<u32>().ok()
        })
        .unwrap_or(
            match path.extension().and_then(|extension| extension.to_str()) {
                Some("svg") => ICON_PIXELS,
                _ => 0,
            },
        )
}
