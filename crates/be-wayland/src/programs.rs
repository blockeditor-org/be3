mod entry;
mod exec;
mod icons;

use std::collections::HashSet;
use std::path::{Path, PathBuf};

pub use entry::{DesktopEntry, locales};
pub use exec::{ExecError, arguments, command};
pub use icons::{IconThemes, load_icon};

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

    pub fn programs(&self) -> Vec<DesktopEntry> {
        let mut seen = HashSet::new();
        let mut out = Vec::new();
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
                let Some(entry) = DesktopEntry::parse(&id, &path, &text, &self.locales) else {
                    continue;
                };
                if entry.shown_in(&self.desktops) && self.runs(&entry) {
                    out.push(entry);
                }
            }
        }
        out.sort_by(|a, b| {
            a.name
                .to_lowercase()
                .cmp(&b.name.to_lowercase())
                .then_with(|| a.id.cmp(&b.id))
        });
        out
    }

    fn runs(&self, entry: &DesktopEntry) -> bool {
        entry
            .try_exec
            .as_ref()
            .is_none_or(|program| self.find_program(program).is_some())
    }

    pub fn command(&self, entry: &DesktopEntry) -> Result<Vec<String>, ExecError> {
        let terminal = match entry.terminal {
            true => self.terminal(),
            false => None,
        };
        command(entry, terminal.as_deref())
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
        if path.is_dir() {
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
