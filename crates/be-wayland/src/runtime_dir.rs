use std::io::{Error, ErrorKind, Result};
use std::os::unix::fs::{DirBuilderExt, MetadataExt};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};

use rustix::io::Errno;
use rustix::process::{Pid, getuid, test_kill_process};

const PREFIX: &str = "be-wayland-";
static MADE: AtomicU32 = AtomicU32::new(0);

pub(crate) struct RuntimeDir {
    path: PathBuf,
    private: bool,
}

impl RuntimeDir {
    pub(crate) fn from_env() -> Result<Self> {
        match std::env::var_os("XDG_RUNTIME_DIR").filter(|dir| !dir.is_empty()) {
            Some(dir) => Ok(Self {
                path: PathBuf::from(dir),
                private: false,
            }),
            None => Self::private_in(&std::env::temp_dir()),
        }
    }

    pub(crate) fn private_in(temp: &Path) -> Result<Self> {
        sweep(temp);
        let path = match MADE.fetch_add(1, Ordering::Relaxed) {
            0 => temp.join(format!("{PREFIX}{}", std::process::id())),
            made => temp.join(format!("{PREFIX}{}-{made}", std::process::id())),
        };
        if let Ok(metadata) = path.symlink_metadata()
            && metadata.uid() == getuid().as_raw()
        {
            remove(&path, metadata.is_dir());
        }
        std::fs::DirBuilder::new().mode(0o700).create(&path)?;
        let metadata = path.symlink_metadata()?;
        if !metadata.is_dir() || metadata.uid() != getuid().as_raw() {
            return Err(Error::new(
                ErrorKind::PermissionDenied,
                format!("{} is not a private directory of ours", path.display()),
            ));
        }
        Ok(Self {
            path,
            private: true,
        })
    }

    pub(crate) fn path(&self) -> &Path {
        &self.path
    }

    pub(crate) fn private(&self) -> bool {
        self.private
    }
}

impl Drop for RuntimeDir {
    fn drop(&mut self) {
        if self.private {
            let _ = std::fs::remove_dir_all(&self.path);
        }
    }
}

pub(crate) fn sweep(temp: &Path) {
    let Ok(entries) = std::fs::read_dir(temp) else {
        return;
    };
    let uid = getuid().as_raw();
    for entry in entries.flatten() {
        let Some(pid) = entry.file_name().to_str().and_then(pid_of) else {
            continue;
        };
        if alive(pid) {
            continue;
        }
        let path = entry.path();
        if let Ok(metadata) = path.symlink_metadata()
            && metadata.uid() == uid
        {
            remove(&path, metadata.is_dir());
        }
    }
}

fn remove(path: &Path, dir: bool) {
    let _ = match dir {
        true => std::fs::remove_dir_all(path),
        false => std::fs::remove_file(path),
    };
}

fn pid_of(name: &str) -> Option<u32> {
    let rest = name.strip_prefix(PREFIX)?;
    let rest = rest.strip_suffix(".lock").unwrap_or(rest);
    let digits = rest
        .split_once('-')
        .map_or(rest, |(pid, made)| match numeric(made) {
            true => pid,
            false => "",
        });
    if !numeric(digits) {
        return None;
    }
    digits.parse().ok()
}

fn numeric(text: &str) -> bool {
    !text.is_empty() && text.bytes().all(|byte| byte.is_ascii_digit())
}

fn alive(pid: u32) -> bool {
    if pid == std::process::id() {
        return true;
    }
    let Some(pid) = i32::try_from(pid).ok().and_then(Pid::from_raw) else {
        return false;
    };
    !matches!(test_kill_process(pid), Err(Errno::SRCH))
}

#[cfg(test)]
mod tests;
