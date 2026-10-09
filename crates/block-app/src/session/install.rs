use std::error::Error;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;

const ENTRY: &str = include_str!("block-app.desktop");
const PAM: &str = include_str!("block-app.pam");
const PAM_DIR: &str = "/etc/pam.d";
const EXECUTABLE: &str = "block-app";
const DEFAULT_PREFIX: &str = "/usr/local";

pub(crate) struct Installed {
    pub(crate) home: PathBuf,
    pub(crate) link: PathBuf,
    pub(crate) entry: PathBuf,
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
    match place(source, &prefix) {
        Ok(installed) => {
            println!(
                "Installed the app in {}, linked it from {} and added the session {}.",
                installed.home.display(),
                installed.link.display(),
                installed.entry.display()
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
                .arg("--install-session")
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
    unsafe { libc::geteuid() == 0 }
}

pub(crate) fn place(source: &Path, prefix: &Path) -> io::Result<Installed> {
    let home = prefix.join("lib").join(EXECUTABLE);
    let link = prefix.join("bin").join(EXECUTABLE);
    let entry = prefix
        .join("share/wayland-sessions")
        .join(format!("{EXECUTABLE}.desktop"));
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
    if let Some(sessions) = entry.parent() {
        std::fs::create_dir_all(sessions)?;
    }
    std::fs::write(&entry, session_entry(&installed))?;
    Ok(Installed { home, link, entry })
}

pub(crate) fn place_pam(dir: &Path) -> io::Result<Option<PathBuf>> {
    let service = dir.join(super::pam::SERVICE);
    if !dir.is_dir() || service.exists() {
        return Ok(None);
    }
    std::fs::write(&service, PAM)?;
    Ok(Some(service))
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
            Some(("Exec", command)) => match command.strip_prefix(EXECUTABLE) {
                Some(arguments) => format!("Exec={}{arguments}", exec_argument(&path)),
                None => line.to_owned(),
            },
            Some(("TryExec", _)) => format!("TryExec={}", path.replace('\\', "\\\\")),
            _ => line.to_owned(),
        })
        .map(|line| line + "\n")
        .collect()
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
