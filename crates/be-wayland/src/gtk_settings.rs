use std::ffi::OsStr;
use std::io::{Error, ErrorKind, Result};
use std::os::unix::fs::DirBuilderExt;
use std::path::{Path, PathBuf};

use gvdb::write::{FileWriter, HashTableBuilder};

const BUTTON_LAYOUT: &str = "/org/gnome/desktop/wm/preferences/button-layout";
const LAYOUT: &str = "appmenu:";

pub(crate) fn profile() -> Option<PathBuf> {
    match write(&directory()) {
        Ok(profile) => Some(profile),
        Err(error) => {
            eprintln!("be-wayland: GTK programs keep their own window buttons: {error}");
            None
        }
    }
}

fn directory() -> PathBuf {
    match std::env::var_os("XDG_RUNTIME_DIR") {
        Some(runtime) => Path::new(&runtime).join("be-wayland"),
        None => {
            std::env::temp_dir().join(format!("be-wayland-{}", rustix::process::getuid().as_raw()))
        }
    }
}

fn write(directory: &Path) -> Result<PathBuf> {
    std::fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(directory)?;
    let database = directory.join("gtk.db");
    let profile = directory.join("dconf.profile");
    replace(&database, &database_contents()?)?;
    let base = base_profile(&profile, std::env::var_os("DCONF_PROFILE").as_deref());
    let layered = format!("{}\nfile-db:{}\n", base.trim_end(), database.display());
    replace(&profile, layered.as_bytes())?;
    Ok(profile)
}

fn database_contents() -> Result<Vec<u8>> {
    let invalid = |error: gvdb::write::Error| Error::new(ErrorKind::InvalidData, error.to_string());
    let mut locks = HashTableBuilder::with_path_separator(None);
    locks.insert_string(BUTTON_LAYOUT, "").map_err(invalid)?;
    let mut table = HashTableBuilder::new();
    table
        .insert_string(BUTTON_LAYOUT, LAYOUT)
        .map_err(invalid)?;
    table.insert_table(".locks", locks).map_err(invalid)?;
    FileWriter::new()
        .write_to_vec_with_table(table)
        .map_err(invalid)
}

fn replace(path: &Path, contents: &[u8]) -> Result<()> {
    let partial = path.with_extension(format!("{}.partial", std::process::id()));
    std::fs::write(&partial, contents)?;
    std::fs::rename(&partial, path)
}

fn base_profile(own: &Path, named: Option<&OsStr>) -> String {
    let file = match named.filter(|name| Path::new(name) != own) {
        Some(name) => profile_file(name),
        None => std::env::var_os("XDG_RUNTIME_DIR")
            .map(|runtime| Path::new(&runtime).join("dconf.profile"))
            .filter(|path| path.is_file())
            .or_else(|| profile_file(OsStr::new("user"))),
    };
    file.and_then(|file| std::fs::read_to_string(file).ok())
        .unwrap_or_else(|| "user-db:user".to_owned())
}

fn profile_file(name: &OsStr) -> Option<PathBuf> {
    if Path::new(name).is_absolute() {
        return Some(PathBuf::from(name));
    }
    let data = std::env::var_os("XDG_DATA_DIRS")
        .filter(|dirs| !dirs.is_empty())
        .unwrap_or_else(|| "/usr/local/share:/usr/share".into());
    std::iter::once(PathBuf::from("/etc"))
        .chain(std::env::split_paths(&data))
        .map(|dir| dir.join("dconf/profile").join(name))
        .find(|path| path.is_file())
}

#[cfg(test)]
mod tests;
