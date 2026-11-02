mod changes;

use std::error::Error;
use std::path::{Path, PathBuf};

pub(crate) use changes::apply_elevated;
use changes::{Access, Change, Decision, Here, User};

const ENTRY: &str = include_str!("be-host.desktop");
const PAM_DIR: &str = super::pam::DIR;
const EXECUTABLE: &str = "be-host";
const DEFAULT_PREFIX: &str = "/usr/local";
const SESSIONS: &str = "usr/share/wayland-sessions";
const LIGHTDM_CONFIGS: [&str; 4] = [
    "usr/share/lightdm/lightdm.conf.d",
    "usr/local/share/lightdm/lightdm.conf.d",
    "etc/xdg/lightdm/lightdm.conf.d",
    "etc/lightdm/lightdm.conf.d",
];
const LIGHTDM_CONFIG: &str = "etc/lightdm/lightdm.conf";
const GREETD_ENVIRONMENTS: &str = "/etc/greetd/environments";
const LIST: &str = "program-files.txt";
const APPS: &str = "apps.json";
const MANIFEST: &str = ".app.json";
const RECEIPT: &str = "installed.json";

pub(crate) struct Layout {
    pub(crate) prefix: PathBuf,
    pub(crate) sessions: Vec<PathBuf>,
    pub(crate) pam: PathBuf,
    pub(crate) greetd: PathBuf,
}

impl Layout {
    fn system(prefix: Option<&str>) -> Result<Self, String> {
        let prefix = PathBuf::from(prefix.unwrap_or(DEFAULT_PREFIX));
        if !prefix.is_absolute() {
            return Err(format!(
                "the prefix {} is not an absolute path",
                prefix.display()
            ));
        }
        Ok(Self {
            prefix,
            sessions: session_dirs(Path::new("/")),
            pam: PathBuf::from(PAM_DIR),
            greetd: PathBuf::from(GREETD_ENVIRONMENTS),
        })
    }

    pub(crate) fn home(&self) -> PathBuf {
        self.prefix.join("lib").join(EXECUTABLE)
    }

    pub(crate) fn link(&self) -> PathBuf {
        self.prefix.join("bin").join(EXECUTABLE)
    }

    fn executable(&self) -> PathBuf {
        self.home().join(EXECUTABLE)
    }
}

#[derive(Debug, Default, PartialEq, Eq)]
pub(crate) struct Receipt {
    pub(crate) files: Vec<PathBuf>,
    pub(crate) link: PathBuf,
    pub(crate) entries: Vec<PathBuf>,
    pub(crate) pam: Option<(PathBuf, String)>,
    pub(crate) greetd: Option<(PathBuf, String)>,
}

impl Receipt {
    pub(crate) fn to_json(&self) -> String {
        let pair = |pair: &Option<(PathBuf, String)>, key: &str| {
            pair.as_ref().map(|(path, value)| {
                serde_json::json!({ "path": path.display().to_string(), key: value })
            })
        };
        let value = serde_json::json!({
            "files": self.files.iter().map(|file| file.display().to_string()).collect::<Vec<_>>(),
            "link": self.link.display().to_string(),
            "entries": self.entries.iter().map(|entry| entry.display().to_string()).collect::<Vec<_>>(),
            "pam": pair(&self.pam, "contents"),
            "greetd": pair(&self.greetd, "line"),
        });
        serde_json::to_string_pretty(&value).unwrap_or_default() + "\n"
    }

    pub(crate) fn from_json(text: &str) -> Option<Self> {
        let value: serde_json::Value = serde_json::from_str(text).ok()?;
        let paths = |key: &str| -> Option<Vec<PathBuf>> {
            value[key]
                .as_array()?
                .iter()
                .map(|path| path.as_str().map(PathBuf::from))
                .collect()
        };
        let pair = |key: &str, field: &str| -> Option<(PathBuf, String)> {
            let pair = &value[key];
            Some((
                PathBuf::from(pair["path"].as_str()?),
                pair[field].as_str()?.to_owned(),
            ))
        };
        Some(Self {
            files: paths("files")?,
            link: PathBuf::from(value["link"].as_str()?),
            entries: paths("entries")?,
            pam: pair("pam", "contents"),
            greetd: pair("greetd", "line"),
        })
    }

    fn read(home: &Path) -> Option<Self> {
        Self::from_json(&std::fs::read_to_string(home.join(RECEIPT)).ok()?)
    }
}

pub(crate) fn install(prefix: Option<&str>) -> Result<(), Box<dyn Error>> {
    let layout = Layout::system(prefix)?;
    let executable = std::env::current_exe()?;
    let source = executable
        .parent()
        .ok_or("be-host's directory is unknown")?;
    let (changes, receipt) = plan_install(source, &layout)?;
    let elevated = run(
        &changes,
        &Here,
        &User::current(),
        &[crate::arguments::INSTALL, &layout.prefix.display().to_string()],
    )?;
    if elevated {
        return Ok(());
    }
    let entries: Vec<String> = receipt
        .entries
        .iter()
        .map(|entry| entry.display().to_string())
        .collect();
    println!(
        "Installed be-host in {}, linked it from {} and added the login session {}.",
        layout.home().display(),
        receipt.link.display(),
        entries.join(" and ")
    );
    if let Some((service, _)) = &receipt.pam {
        println!(
            "The lock screen checks passwords with {}.",
            service.display()
        );
    }
    if let Some((environments, _)) = &receipt.greetd {
        println!(
            "The login session is listed in {}, which greetd's greeters list.",
            environments.display()
        );
    }
    println!("Log out and pick Block from the session menu on the login screen.");
    Ok(())
}

pub(crate) fn uninstall(prefix: Option<&str>) -> Result<(), Box<dyn Error>> {
    let layout = Layout::system(prefix)?;
    let changes = plan_uninstall(&layout)?;
    let elevated = run(
        &changes,
        &Here,
        &User::current(),
        &[
            crate::arguments::UNINSTALL,
            &layout.prefix.display().to_string(),
        ],
    )?;
    if elevated {
        return Ok(());
    }
    let home = layout.home();
    match home.exists() {
        true => println!(
            "Uninstalled be-host's login session, and left {}, which holds files the install did not put there.",
            home.display()
        ),
        false => println!(
            "Uninstalled be-host's login session from {}.",
            layout.prefix.display()
        ),
    }
    Ok(())
}

fn run(
    changes: &[Change],
    access: &dyn Access,
    user: &User,
    again: &[&str],
) -> Result<bool, Box<dyn Error>> {
    match changes::decide(changes, access, user)? {
        Decision::Here => {
            changes::apply(changes, &[], |_| Ok(()))?;
            Ok(false)
        }
        Decision::Elevate { whole: true, blocked } => {
            changes::sudo_can_ask(&blocked)?;
            println!(
                "{} needs root, so it all runs as root with sudo.",
                changes::list(&blocked)
            );
            changes::sudo_again(again)?;
            Ok(true)
        }
        Decision::Elevate {
            whole: false,
            blocked,
        } => {
            changes::sudo_can_ask(&blocked)?;
            println!(
                "{} needs root, so sudo writes that and the rest is written as you.",
                changes::list(&blocked)
            );
            let elevated: Vec<bool> = changes
                .iter()
                .map(|change| change.blocked(access).is_some())
                .collect();
            changes::apply(changes, &elevated, changes::sudo_apply)?;
            Ok(false)
        }
    }
}

pub(crate) fn program_files(source: &Path) -> Result<Vec<PathBuf>, String> {
    let list = std::fs::read_to_string(source.join(LIST)).map_err(|_| {
        format!(
            "{} has no {LIST}, the list of what its build put there, so it is not be-host built with its apps (//crates/be-host:native)",
            source.display()
        )
    })?;
    let mut files = Vec::new();
    for line in list.lines().filter(|line| !line.is_empty()) {
        let file = PathBuf::from(line);
        let inside = file
            .components()
            .all(|component| matches!(component, std::path::Component::Normal(_)));
        if !inside {
            return Err(format!(
                "{LIST} in {} lists {line}, which is not inside it",
                source.display()
            ));
        }
        if !source.join(&file).is_file() {
            return Err(format!(
                "{} is missing {line}, which its build put there",
                source.display()
            ));
        }
        files.push(file);
    }
    let listed = |name: &str| files.iter().any(|file| file == Path::new(name));
    for required in [EXECUTABLE, APPS] {
        if !listed(required) {
            return Err(format!(
                "{} has no {required}, so it is not be-host built with its apps (//crates/be-host:native)",
                source.display()
            ));
        }
    }
    let apps: Vec<String> = std::fs::read_to_string(source.join(APPS))
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .ok_or_else(|| format!("{} in {} is not a list of apps", APPS, source.display()))?;
    if apps.is_empty() {
        return Err(format!("{} has no apps", source.display()));
    }
    for app in apps {
        if !app.ends_with(MANIFEST) || !listed(&app) {
            return Err(format!(
                "{} lists the app {app}, which is not there",
                source.display()
            ));
        }
    }
    files.push(PathBuf::from(LIST));
    Ok(files)
}

pub(crate) fn plan_install(
    source: &Path,
    layout: &Layout,
) -> Result<(Vec<Change>, Receipt), String> {
    let files = program_files(source)?;
    let home = layout.home();
    if home.exists() && !home.join(EXECUTABLE).is_file() {
        return Err(format!(
            "{} is in the way and is not an installed be-host",
            home.display()
        ));
    }
    let earlier = Receipt::read(&home).unwrap_or_default();
    let mut changes = Vec::new();
    let same = std::fs::canonicalize(source).ok() == std::fs::canonicalize(&home).ok();
    if !same {
        changes.push(Change::Copy {
            source: source.to_path_buf(),
            files: files.clone(),
            home: home.clone(),
        });
    }
    let installed = layout.executable();
    let link = layout.link();
    changes.push(Change::Link {
        link: link.clone(),
        target: installed.clone(),
    });
    let name = format!("{EXECUTABLE}.desktop");
    let entries: Vec<PathBuf> = layout.sessions.iter().map(|dir| dir.join(&name)).collect();
    for entry in &entries {
        changes.push(Change::Write {
            path: entry.clone(),
            contents: session_entry(&installed),
        });
    }
    let older = layout.prefix.join("share/wayland-sessions").join(&name);
    for stale in earlier.entries.iter().chain([&older]) {
        if !entries.contains(stale) && std::fs::symlink_metadata(stale).is_ok() {
            changes.push(Change::Remove {
                path: stale.clone(),
            });
        }
    }
    let pam = match pam_to_place(&layout.pam) {
        Some((service, contents)) => {
            changes.push(Change::Write {
                path: service.clone(),
                contents: contents.clone(),
            });
            Some((service, contents))
        }
        None => earlier.pam.filter(|(service, contents)| {
            std::fs::read_to_string(service).ok().as_ref() == Some(contents)
        }),
    };
    let greetd = std::fs::read_to_string(&layout.greetd)
        .ok()
        .and_then(|listed| greetd_listing(&listed, &link))
        .and_then(|listing| {
            if let Some(updated) = listing.updated {
                changes.push(Change::Write {
                    path: layout.greetd.clone(),
                    contents: updated,
                });
            }
            let ours = listing.added || earlier.greetd.is_some();
            ours.then(|| (layout.greetd.clone(), listing.line))
        });
    let receipt = Receipt {
        files,
        link,
        entries,
        pam,
        greetd,
    };
    changes.push(Change::Write {
        path: home.join(RECEIPT),
        contents: receipt.to_json(),
    });
    Ok((changes, receipt))
}

pub(crate) fn plan_uninstall(layout: &Layout) -> Result<Vec<Change>, String> {
    let home = layout.home();
    let receipt = Receipt::read(&home).ok_or_else(|| {
        format!(
            "{} has no record of an install ({RECEIPT}), so there is nothing to uninstall",
            home.display()
        )
    })?;
    let mut changes = Vec::new();
    for entry in &receipt.entries {
        if std::fs::symlink_metadata(entry).is_ok() {
            changes.push(Change::Remove {
                path: entry.clone(),
            });
        }
    }
    if std::fs::read_link(&receipt.link).ok() == Some(layout.executable()) {
        changes.push(Change::Remove {
            path: receipt.link.clone(),
        });
    }
    if let Some((service, contents)) = &receipt.pam
        && std::fs::read_to_string(service).ok().as_ref() == Some(contents)
    {
        changes.push(Change::Remove {
            path: service.clone(),
        });
    }
    if let Some((environments, line)) = &receipt.greetd
        && let Ok(listed) = std::fs::read_to_string(environments)
        && listed.lines().any(|listed| listed.trim() == line)
    {
        let kept: String = listed
            .lines()
            .filter(|listed| listed.trim() != line)
            .map(|listed| format!("{listed}\n"))
            .collect();
        changes.push(Change::Write {
            path: environments.clone(),
            contents: kept,
        });
    }
    let mut files = receipt.files;
    files.push(PathBuf::from(RECEIPT));
    changes.push(Change::Clear { home, files });
    Ok(changes)
}

pub(crate) fn session_dirs(root: &Path) -> Vec<PathBuf> {
    let standard = root.join(SESSIONS);
    let mut dirs = vec![standard.clone()];
    let Some(configured) = lightdm_sessions(root) else {
        return dirs;
    };
    let configured: Vec<PathBuf> = configured
        .split(':')
        .map(str::trim)
        .filter(|dir| dir.starts_with('/'))
        .map(|dir| root.join(dir.trim_start_matches('/')))
        .collect();
    if !configured.contains(&standard)
        && let Some(first) = configured.into_iter().next()
    {
        dirs.push(first);
    }
    dirs
}

fn lightdm_sessions(root: &Path) -> Option<String> {
    let mut files = Vec::new();
    for dir in LIGHTDM_CONFIGS {
        let Ok(entries) = std::fs::read_dir(root.join(dir)) else {
            continue;
        };
        let mut found: Vec<PathBuf> = entries
            .filter_map(|entry| entry.ok().map(|entry| entry.path()))
            .filter(|path| {
                path.extension()
                    .is_some_and(|extension| extension == "conf")
            })
            .collect();
        found.sort();
        files.extend(found);
    }
    files.push(root.join(LIGHTDM_CONFIG));
    let mut sessions = None;
    for file in files {
        let Ok(text) = std::fs::read_to_string(file) else {
            continue;
        };
        let mut section = String::new();
        for line in text.lines().map(str::trim) {
            if let Some(name) = line
                .strip_prefix('[')
                .and_then(|line| line.strip_suffix(']'))
            {
                section = name.trim().to_owned();
            } else if section == "LightDM"
                && let Some((key, value)) = line.split_once('=')
                && key.trim() == "sessions-directory"
            {
                sessions = Some(value.trim().to_owned());
            }
        }
    }
    sessions
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct GreetdListing {
    pub(crate) updated: Option<String>,
    pub(crate) line: String,
    pub(crate) added: bool,
}

pub(crate) fn greetd_listing(listed: &str, link: &Path) -> Option<GreetdListing> {
    let path = link.display().to_string();
    if path.contains(char::is_whitespace) {
        return None;
    }
    let line = command(&path);
    let stale = format!("{path} --session");
    let kept: Vec<&str> = listed.lines().filter(|listed| listed.trim() != stale).collect();
    let listed_already = kept.iter().any(|listed| listed.trim() == line);
    let changed = !listed_already || kept.len() != listed.lines().count();
    let updated = changed.then(|| {
        let mut updated: String = kept.iter().map(|listed| format!("{listed}\n")).collect();
        if !listed_already {
            updated.push_str(&line);
            updated.push('\n');
        }
        updated
    });
    Some(GreetdListing {
        updated,
        line,
        added: !listed_already,
    })
}

pub(crate) fn pam_to_place(dir: &Path) -> Option<(PathBuf, String)> {
    let service = dir.join(super::pam::SERVICE);
    if !dir.is_dir() || std::fs::symlink_metadata(&service).is_ok() {
        return None;
    }
    Some((service, pam_service(dir)))
}

pub(crate) fn pam_service(dir: &Path) -> String {
    let base = match dir.join("common-auth").is_file() {
        true => "common-auth",
        false => super::pam::FALLBACK_SERVICE,
    };
    format!("auth include {base}\n")
}

pub(crate) fn session_entry(executable: &Path) -> String {
    let path = executable.display().to_string();
    ENTRY
        .lines()
        .map(|line| match line.split_once('=') {
            Some(("Exec", _)) => format!("Exec={}", command(&exec_argument(&path))),
            Some(("TryExec", _)) => format!("TryExec={}", path.replace('\\', "\\\\")),
            _ => line.to_owned(),
        })
        .map(|line| line + "\n")
        .collect()
}

fn command(executable: &str) -> String {
    let exec = ENTRY
        .lines()
        .find_map(|line| line.strip_prefix("Exec="))
        .unwrap_or(EXECUTABLE);
    exec.split(' ')
        .map(|word| match word {
            EXECUTABLE => executable,
            word => word,
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn exec_argument(path: &str) -> String {
    let plain = path
        .chars()
        .all(|character| character.is_ascii_alphanumeric() || "/._-+".contains(character));
    if plain {
        return path.to_owned();
    }
    let mut quoted = String::from("\"");
    for character in path.chars() {
        match character {
            '"' | '`' | '$' => {
                quoted.push_str("\\\\");
                quoted.push(character);
            }
            '\\' => quoted.push_str("\\\\\\\\"),
            '%' => quoted.push_str("%%"),
            _ => quoted.push(character),
        }
    }
    quoted.push('"');
    quoted
}

#[cfg(test)]
pub(crate) fn apply_here(changes: &[Change]) -> std::io::Result<()> {
    changes::apply(changes, &[], |_| Ok(())).map_err(|error| std::io::Error::other(error.to_string()))
}

#[cfg(test)]
mod tests;
