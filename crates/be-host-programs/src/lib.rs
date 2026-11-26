//! The programs installed on a Linux desktop as freedesktop.org describes them: desktop
//! entries, how to run them, the MIME types they open and their icons in the icon themes.

mod entry;
mod exec;
mod icons;
mod mime;
mod xpm;

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

pub use entry::{DesktopEntry, locales};
pub use exec::{ExecError, arguments, command};
pub use icons::{IconThemes, NamedIcons, load_icon};
pub use mime::MimeGlobs;

const TERMINALS: [(&str, &[&str]); 8] = [
    ("foot", &[]),
    ("kitty", &[]),
    ("alacritty", &["-e"]),
    ("wezterm", &["start", "--"]),
    ("gnome-terminal", &["--"]),
    ("konsole", &["-e"]),
    ("xfce4-terminal", &["-x"]),
    ("xterm", &["-e"]),
];

#[derive(Clone, Debug, Default)]
pub struct Environment {
    pub data_dirs: Vec<PathBuf>,
    pub desktops: Vec<String>,
    pub locales: Vec<String>,
    pub path: Vec<PathBuf>,
    pub home: Option<PathBuf>,
    pub config_home: Option<PathBuf>,
    pub terminal: Option<String>,
}

impl Environment {
    pub fn from_env() -> Self {
        let var = |name: &str| std::env::var(name).ok().filter(|value| !value.is_empty());
        let home = var("HOME").map(PathBuf::from);
        let data_home = var("XDG_DATA_HOME")
            .map(PathBuf::from)
            .or_else(|| home.as_ref().map(|home| home.join(".local/share")));
        let data_dirs =
            var("XDG_DATA_DIRS").unwrap_or_else(|| "/usr/local/share:/usr/share".into());
        let messages = var("LC_ALL")
            .or_else(|| var("LC_MESSAGES"))
            .or_else(|| var("LANG"))
            .unwrap_or_default();
        Self {
            data_dirs: data_home
                .into_iter()
                .chain(std::env::split_paths(&data_dirs))
                .collect(),
            desktops: var("XDG_CURRENT_DESKTOP")
                .map(|desktops| desktops.split(':').map(str::to_owned).collect())
                .unwrap_or_default(),
            locales: locales(&messages),
            path: var("PATH")
                .map(|path| std::env::split_paths(&path).collect())
                .unwrap_or_default(),
            home,
            config_home: var("XDG_CONFIG_HOME").map(PathBuf::from),
            terminal: var("TERMINAL"),
        }
    }

    fn config_home(&self) -> Option<PathBuf> {
        self.config_home
            .clone()
            .or_else(|| self.home.as_ref().map(|home| home.join(".config")))
    }

    pub fn find_program(&self, name: &str) -> Option<PathBuf> {
        if name.contains('/') {
            return executable(Path::new(name)).then(|| PathBuf::from(name));
        }
        self.path
            .iter()
            .map(|dir| dir.join(name))
            .find(|candidate| executable(candidate))
    }

    pub fn terminal(&self) -> Option<Vec<String>> {
        if let Some(terminal) = &self.terminal
            && self.find_program(terminal).is_some()
        {
            return Some(vec![terminal.clone(), "-e".to_owned()]);
        }
        TERMINALS
            .iter()
            .find(|(name, _)| self.find_program(name).is_some())
            .map(|(name, prefix)| {
                std::iter::once(*name)
                    .chain(prefix.iter().copied())
                    .map(str::to_owned)
                    .collect()
            })
    }

    /// Every desktop entry installed, hidden or not: the first of each id in the data folders'
    /// order, each folder's in the order of its ids.
    pub fn entries(&self) -> Entries {
        let mut seen = HashSet::new();
        let mut entries = Entries::default();
        for dir in &self.data_dirs {
            let applications = dir.join("applications");
            let mut found = Vec::new();
            collect(&applications, &applications, &mut found);
            found.sort();
            for (id, path) in found {
                if !seen.insert(id.clone()) {
                    continue;
                }
                let Ok(text) = std::fs::read_to_string(&path) else {
                    continue;
                };
                let path = path.to_string_lossy();
                if let Some(entry) = DesktopEntry::parse(&id, &path, &text, &self.locales) {
                    entries.push(entry);
                }
            }
        }
        entries
    }

    /// The programs a person may run, of the installed `entries`: those shown on this desktop
    /// whose `TryExec` is found, by name.
    pub fn programs_among(&self, entries: &Entries) -> Vec<DesktopEntry> {
        let mut out: Vec<DesktopEntry> = entries
            .all
            .iter()
            .filter(|entry| entry.shown_in(&self.desktops) && self.runs(entry))
            .cloned()
            .collect();
        out.sort_by(|a, b| {
            a.name
                .to_lowercase()
                .cmp(&b.name.to_lowercase())
                .then_with(|| a.id.cmp(&b.id))
        });
        out
    }

    pub fn programs(&self) -> Vec<DesktopEntry> {
        self.programs_among(&self.entries())
    }

    fn runs(&self, entry: &DesktopEntry) -> bool {
        entry
            .try_exec
            .as_ref()
            .is_none_or(|program| self.find_program(program).is_some())
    }

    pub fn command(
        &self,
        entry: &DesktopEntry,
        file: Option<&str>,
    ) -> Result<Vec<String>, ExecError> {
        let terminal = match entry.terminal {
            true => self.terminal(),
            false => None,
        };
        command(entry, terminal.as_deref(), file)
    }

    pub fn mime_globs(&self) -> MimeGlobs {
        MimeGlobs::read(&self.data_dirs)
    }
}

fn executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    std::fs::metadata(path)
        .is_ok_and(|metadata| metadata.is_file() && metadata.permissions().mode() & 0o111 != 0)
}

fn collect(root: &Path, dir: &Path, found: &mut Vec<(String, PathBuf)>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if entry.file_type().is_ok_and(|kind| kind.is_dir()) {
            collect(root, &path, found);
            continue;
        }
        if path
            .extension()
            .is_none_or(|extension| extension != "desktop")
        {
            continue;
        }
        let Ok(relative) = path.strip_prefix(root) else {
            continue;
        };
        let id = relative
            .components()
            .map(|part| part.as_os_str().to_string_lossy())
            .collect::<Vec<_>>()
            .join("-");
        found.push((id, path));
    }
}

#[cfg(test)]
mod tests;

/// The desktop entries installed (`Environment::entries`), found by the id or class a window
/// names.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Entries {
    all: Vec<DesktopEntry>,
    /// Each id's entry.
    by_id: HashMap<String, usize>,
    /// The first entry whose `StartupWMClass` is each class, in lower case.
    by_class: HashMap<String, usize>,
}

impl Entries {
    fn push(&mut self, entry: DesktopEntry) {
        let at = self.all.len();
        self.by_id.insert(entry.id.clone(), at);
        if let Some(class) = &entry.startup_wm_class {
            self.by_class.entry(class.to_ascii_lowercase()).or_insert(at);
        }
        self.all.push(entry);
    }

    pub fn all(&self) -> &[DesktopEntry] {
        &self.all
    }

    /// The entry with `id` (`org.example.App.desktop`; a subfolder's `kde-kate.desktop`).
    pub fn get(&self, id: &str) -> Option<&DesktopEntry> {
        self.by_id.get(id).map(|at| &self.all[*at])
    }

    /// The entry a window's app id or X11 class names: the entry it is the id of, as it is or
    /// in lower case, else the first whose `StartupWMClass` it is in any case.
    pub fn of_window(&self, program_id: &str) -> Option<&DesktopEntry> {
        if program_id.is_empty() {
            return None;
        }
        self.get(&format!("{program_id}.desktop"))
            .or_else(|| self.get(&format!("{}.desktop", program_id.to_lowercase())))
            .or_else(|| {
                let at = self.by_class.get(&program_id.to_ascii_lowercase())?;
                Some(&self.all[*at])
            })
    }
}
