use std::fs::File;
use std::fs::{OpenOptions, Permissions};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::os::fd::{AsFd, AsRawFd, FromRawFd, OwnedFd};
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

const LOG: &str = "login-session";
const PREVIOUS: &str = "login-session.previous";

#[derive(Debug, Clone, Copy)]
pub(crate) struct Cap {
    pub(crate) limit: u64,
    pub(crate) head: u64,
    pub(crate) parts: usize,
}

const CAP: Cap = Cap {
    limit: 4 << 20,
    head: 256 << 10,
    parts: 2,
};

pub(crate) fn log_path(directory: &Path, name: &str, part: usize) -> PathBuf {
    match part {
        0 => directory.join(format!("{name}.log")),
        part => directory.join(format!("{name}.{part}.log")),
    }
}

pub(crate) fn keep_previous(directory: &Path, cap: Cap) {
    for part in 0..=cap.parts {
        let _ = std::fs::remove_file(log_path(directory, PREVIOUS, part));
        let _ = std::fs::rename(
            log_path(directory, LOG, part),
            log_path(directory, PREVIOUS, part),
        );
    }
}

impl Cap {
    pub(crate) fn check(&self, log: &mut File, directory: &Path) -> io::Result<bool> {
        let size = log.metadata()?.len();
        if size <= self.limit {
            return Ok(false);
        }
        let path = log_path(directory, LOG, 0);
        let mut reader = File::open(&path)?;
        let mut head = Vec::new();
        (&mut reader).take(self.head).read_to_end(&mut head)?;
        let cut = match head.iter().rposition(|byte| *byte == b'\n') {
            Some(end) => end as u64 + 1,
            None => head.len() as u64,
        };
        let _ = std::fs::remove_file(log_path(directory, LOG, self.parts));
        for part in (1..self.parts).rev() {
            let _ = std::fs::rename(
                log_path(directory, LOG, part),
                log_path(directory, LOG, part + 1),
            );
        }
        let moved = log_path(directory, LOG, 1);
        let mut part = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .mode(0o600)
            .open(&moved)?;
        reader.seek(SeekFrom::Start(cut))?;
        let mut copied = cut;
        loop {
            copied += io::copy(&mut reader, &mut part)?;
            if log.metadata()?.len() <= copied {
                log.set_len(cut)?;
                break;
            }
        }
        writeln!(
            log,
            "be-host: the log passed {} bytes, so what followed its start up to here is in {}.",
            self.limit,
            moved.display()
        )?;
        Ok(true)
    }
}

pub(crate) fn start_log() -> Option<File> {
    let Some(directory) = state_directory() else {
        eprintln!("be-host: no directory for the login session's log; set XDG_STATE_HOME or HOME");
        return None;
    };
    let path = log_path(&directory, LOG, 0);
    let opened = std::fs::create_dir_all(&directory).and_then(|()| {
        std::fs::set_permissions(&directory, Permissions::from_mode(0o700))?;
        keep_previous(&directory, CAP);
        OpenOptions::new()
            .create(true)
            .append(true)
            .mode(0o600)
            .open(&path)
    });
    let mut file = match opened {
        Ok(file) => file,
        Err(error) => {
            eprintln!(
                "be-host: the login session's log {} did not open: {error}",
                path.display()
            );
            return None;
        }
    };
    let started = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let _ = writeln!(
        file,
        "be-host {} started a login session at {started} seconds after the Unix epoch.",
        crate::build()
    );
    eprintln!("be-host: the login session logs to {}", path.display());
    let console = std::io::stderr()
        .as_fd()
        .try_clone_to_owned()
        .ok()
        .map(File::from);
    for stream in [libc::STDOUT_FILENO, libc::STDERR_FILENO] {
        // unsafe(ffi)
        unsafe { libc::dup2(file.as_raw_fd(), stream) };
    }
    if let Err(error) = watch(file, directory, &path) {
        eprintln!("be-host: the login session's log is not capped: {error}");
    }
    console
}

fn watch(mut log: File, directory: PathBuf, path: &Path) -> io::Result<()> {
    // unsafe(ffi)
    let raw = unsafe { libc::inotify_init1(libc::IN_CLOEXEC) };
    if raw < 0 {
        return Err(io::Error::last_os_error());
    }
    // unsafe(ffi)
    let events = unsafe { OwnedFd::from_raw_fd(raw) };
    let name = std::ffi::CString::new(path.as_os_str().as_bytes()).map_err(io::Error::other)?;
    // unsafe(ffi)
    if unsafe { libc::inotify_add_watch(raw, name.as_ptr(), libc::IN_MODIFY) } < 0 {
        return Err(io::Error::last_os_error());
    }
    let mut events = File::from(events);
    std::thread::Builder::new()
        .name("log cap".to_owned())
        .spawn(move || {
            let mut buffer = [0u8; 4096];
            while events.read(&mut buffer).is_ok_and(|read| read > 0) {
                if let Err(error) = CAP.check(&mut log, &directory) {
                    eprintln!("be-host: capping the login session's log failed: {error}");
                    return;
                }
            }
        })?;
    Ok(())
}

/// Where be-host keeps what it writes about a run: `$XDG_STATE_HOME` (or `~/.local/state`) under the
/// app id in lower case.
pub(crate) fn state_directory() -> Option<PathBuf> {
    let state = std::env::var_os("XDG_STATE_HOME")
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
        .or_else(|| {
            std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".local/state"))
        })?;
    Some(state.join(crate::program_id().to_lowercase()))
}

#[cfg(test)]
mod tests;
