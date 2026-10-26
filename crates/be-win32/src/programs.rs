use std::collections::HashSet;
use std::path::{Path, PathBuf};

use crate::command_line::split_arguments;

/// What a `.lnk` file says, as `IShellLinkW` resolves it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Shortcut {
    pub target: Option<PathBuf>,
    pub arguments: String,
    pub description: String,
    pub app_user_model_id: Option<String>,
}

/// A Start Menu shortcut.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Program {
    /// The shortcut's path below the Programs folder, without `.lnk`, with
    /// `/` between folders: `Accessories/Paint`.
    pub id: String,
    /// The file's name without `.lnk`, which is what the Start Menu shows.
    pub name: String,
    /// The Start Menu folder it is in, if any: `Accessories`.
    pub folder: Option<String>,
    /// The `.lnk` file, which is what a launch opens.
    pub path: PathBuf,
    pub shortcut: Shortcut,
}

impl Program {
    /// The command the shortcut runs, when it points at a file.
    pub fn command(&self) -> Vec<String> {
        let Some(target) = &self.shortcut.target else {
            return Vec::new();
        };
        std::iter::once(target.to_string_lossy().into_owned())
            .chain(split_arguments(&self.shortcut.arguments))
            .collect()
    }
}

/// The programs of the Start Menu's Programs folders, the user's first then
/// everyone's: every `.lnk` below them that `resolve` can read, but
/// uninstallers. A shortcut at the same place in a later folder, or to the
/// same command under the same name, is the earlier one again. Sorted by
/// name without case, then by id.
pub fn list(folders: &[PathBuf], mut resolve: impl FnMut(&Path) -> Option<Shortcut>) -> Vec<Program> {
    let mut seen_ids = HashSet::new();
    let mut seen_commands = HashSet::new();
    let mut programs = Vec::new();
    for folder in folders {
        let mut found = Vec::new();
        collect(folder, &mut Vec::new(), &mut found);
        found.sort();
        for (parts, path) in found {
            let id = parts.join("/");
            if !seen_ids.insert(id.to_lowercase()) {
                continue;
            }
            let name = parts.last().cloned().unwrap_or_default();
            let Some(shortcut) = resolve(&path) else {
                continue;
            };
            if uninstaller(&name, &shortcut) {
                continue;
            }
            let command = (
                name.to_lowercase(),
                shortcut
                    .target
                    .as_ref()
                    .map(|target| target.to_string_lossy().to_lowercase()),
                shortcut.arguments.clone(),
                shortcut.app_user_model_id.clone(),
            );
            if !seen_commands.insert(command) {
                continue;
            }
            let folder = (parts.len() > 1).then(|| parts[..parts.len() - 1].join("/"));
            programs.push(Program {
                id,
                name,
                folder,
                path,
                shortcut,
            });
        }
    }
    programs.sort_by(|a, b| {
        a.name
            .to_lowercase()
            .cmp(&b.name.to_lowercase())
            .then_with(|| a.id.cmp(&b.id))
    });
    programs
}

fn uninstaller(name: &str, shortcut: &Shortcut) -> bool {
    let target = shortcut
        .target
        .as_deref()
        .map(|target| target.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    let target = target.rsplit(['\\', '/']).next().unwrap_or_default();
    name.to_lowercase().contains("uninstall") || target.starts_with("unins")
}

fn collect(dir: &Path, parts: &mut Vec<String>, found: &mut Vec<(Vec<String>, PathBuf)>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Some(file) = path.file_name().map(|file| file.to_string_lossy().into_owned()) else {
            continue;
        };
        if entry.file_type().is_ok_and(|kind| kind.is_dir()) {
            parts.push(file);
            collect(&path, parts, found);
            parts.pop();
            continue;
        }
        let Some(stem) = file
            .len()
            .checked_sub(4)
            .filter(|&at| file.is_char_boundary(at) && file[at..].eq_ignore_ascii_case(".lnk"))
            .map(|at| file[..at].to_owned())
        else {
            continue;
        };
        let mut name = parts.clone();
        name.push(stem);
        found.push((name, path));
    }
}
