use std::error::Error;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;

const ENTRY: &str = include_str!("block-app.desktop");
const PAM_DIR: &str = super::pam::DIR;
const EXECUTABLE: &str = "block-app";
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

pub(crate) struct Installed {
    pub(crate) home: PathBuf,
    pub(crate) link: PathBuf,
    pub(crate) entries: Vec<PathBuf>,
}

pub(crate) fn install(prefix: Option<&str>) -> Result<(), Box<dyn Error>> {
    let prefix = PathBuf::from(prefix.unwrap_or(DEFAULT_PREFIX));
    if !prefix.is_absolute() {
        return Err(format!("the prefix {} is not an absolute path", prefix.display()).into());
    }
    let executable = std::env::current_exe()?;
    let source = executable
        .parent()
        .ok_or("the app's directory is unknown")?;
    match place(source, &prefix, &session_dirs(Path::new("/"))) {
        Ok(installed) => {
            let entries: Vec<String> = installed
                .entries
                .iter()
                .map(|entry| entry.display().to_string())
                .collect();
            println!(
                "Installed the app in {}, linked it from {} and added the login session {}.",
                installed.home.display(),
                installed.link.display(),
                entries.join(" and ")
            );
            match place_pam(Path::new(PAM_DIR)) {
                Ok(Some(service)) => {
                    println!(
                        "Added {}, which the lock screen checks passwords with.",
                        service.display()
                    );
                }
                Ok(None) => {}
                Err(error) => println!(
                    "Could not add the lock screen's PAM service to {PAM_DIR} ({error}), so it checks passwords with the login service."
                ),
            }
            match place_greetd(Path::new(GREETD_ENVIRONMENTS), &installed.link) {
                Ok(true) => println!(
                    "Added the login session to {GREETD_ENVIRONMENTS}, which greetd's greeters list."
                ),
                Ok(false) => {}
                Err(error) => println!(
                    "Could not add the login session to {GREETD_ENVIRONMENTS} ({error}), so greetd's greeters do not list it."
                ),
            }
            println!("Log out and pick Block from the session menu on the login screen.");
            Ok(())
        }
        Err(error) if error.kind() == io::ErrorKind::PermissionDenied && !root() => {
            println!(
                "Installing under {} needs root, so it asks sudo.",
                prefix.display()
            );
            let status = Command::new("sudo")
                .arg(&executable)
                .arg("--install-login-session")
                .arg(&prefix)
                .status()?;
            match status.success() {
                true => Ok(()),
                false => Err(format!("the install under sudo failed: {status}").into()),
            }
        }
        Err(error) => Err(error.into()),
    }
}

fn root() -> bool {
    // unsafe(ffi)
    unsafe { libc::geteuid() == 0 }
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

pub(crate) fn place(source: &Path, prefix: &Path, sessions: &[PathBuf]) -> io::Result<Installed> {
    let home = prefix.join("lib").join(EXECUTABLE);
    let link = prefix.join("bin").join(EXECUTABLE);
    let name = format!("{EXECUTABLE}.desktop");
    if !source.join(EXECUTABLE).is_file() {
        return Err(io::Error::other(format!(
            "{} has no {EXECUTABLE} to install",
            source.display()
        )));
    }
    let same = std::fs::canonicalize(source).ok() == std::fs::canonicalize(&home).ok();
    if !same {
        if home.exists() && !home.join(EXECUTABLE).is_file() {
            return Err(io::Error::other(format!(
                "{} is in the way and is not an installed app",
                home.display()
            )));
        }
        replace_tree(source, &home)?;
    }
    let installed = home.join(EXECUTABLE);
    std::fs::create_dir_all(prefix.join("bin"))?;
    if std::fs::symlink_metadata(&link).is_ok() {
        std::fs::remove_file(&link)?;
    }
    std::os::unix::fs::symlink(&installed, &link)?;
    let mut entries = Vec::new();
    for dir in sessions {
        std::fs::create_dir_all(dir)?;
        let entry = dir.join(&name);
        std::fs::write(&entry, session_entry(&installed))?;
        entries.push(entry);
    }
    let old = prefix.join("share/wayland-sessions");
    if !sessions.contains(&old) && old.join(&name).is_file() {
        std::fs::remove_file(old.join(&name))?;
    }
    Ok(Installed {
        home,
        link,
        entries,
    })
}

pub(crate) fn place_greetd(environments: &Path, link: &Path) -> io::Result<bool> {
    let Ok(listed) = std::fs::read_to_string(environments) else {
        return Ok(false);
    };
    let path = link.display().to_string();
    if path.contains(char::is_whitespace) {
        return Ok(false);
    }
    let command = command(&path);
    if listed.lines().any(|line| line.trim() == command) {
        return Ok(false);
    }
    let mut updated = listed;
    if !updated.is_empty() && !updated.ends_with('\n') {
        updated.push('\n');
    }
    updated.push_str(&command);
    updated.push('\n');
    std::fs::write(environments, updated)?;
    Ok(true)
}

pub(crate) fn place_pam(dir: &Path) -> io::Result<Option<PathBuf>> {
    let service = dir.join(super::pam::SERVICE);
    if !dir.is_dir() || service.exists() {
        return Ok(None);
    }
    std::fs::write(&service, pam_service(dir))?;
    Ok(Some(service))
}

pub(crate) fn pam_service(dir: &Path) -> String {
    let base = match dir.join("common-auth").is_file() {
        true => "common-auth",
        false => super::pam::FALLBACK_SERVICE,
    };
    format!("auth include {base}\n")
}

fn replace_tree(source: &Path, home: &Path) -> io::Result<()> {
    let staged = home.with_extension("new");
    let old = home.with_extension("old");
    for leftover in [&staged, &old] {
        if leftover.exists() {
            std::fs::remove_dir_all(leftover)?;
        }
    }
    if let Err(error) = copy_tree(source, &staged) {
        let _ = std::fs::remove_dir_all(&staged);
        return Err(error);
    }
    let kept = home.exists();
    if kept {
        std::fs::rename(home, &old)?;
    }
    if let Err(error) = std::fs::rename(&staged, home) {
        if kept {
            let _ = std::fs::rename(&old, home);
        }
        return Err(error);
    }
    if kept {
        std::fs::remove_dir_all(&old)?;
    }
    Ok(())
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

fn copy_tree(from: &Path, to: &Path) -> io::Result<()> {
    std::fs::create_dir_all(to)?;
    for entry in std::fs::read_dir(from)? {
        let entry = entry?;
        let target = to.join(entry.file_name());
        let kind = std::fs::metadata(entry.path())?;
        if kind.is_dir() {
            copy_tree(&entry.path(), &target)?;
        } else {
            std::fs::copy(entry.path(), &target)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests;
