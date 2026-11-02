use std::fs::File;
use std::fs::{OpenOptions, Permissions};
use std::io::Write;
use std::os::fd::{AsFd, AsRawFd};
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

const LOG: &str = "login-session.log";
const PREVIOUS_LOG: &str = "login-session.previous.log";

pub(crate) fn start_log() -> Option<File> {
    let Some(directory) = log_directory() else {
        eprintln!(
            "block-app: no directory for the login session's log; set XDG_STATE_HOME or HOME"
        );
        return None;
    };
    let path = directory.join(LOG);
    let opened = std::fs::create_dir_all(&directory).and_then(|()| {
        std::fs::set_permissions(&directory, Permissions::from_mode(0o700))?;
        let _ = std::fs::rename(&path, directory.join(PREVIOUS_LOG));
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
                "block-app: the login session's log {} did not open: {error}",
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
        "Block {} started a login session at {started} seconds after the Unix epoch.",
        crate::build()
    );
    eprintln!("block-app: the login session logs to {}", path.display());
    let console = std::io::stderr()
        .as_fd()
        .try_clone_to_owned()
        .ok()
        .map(File::from);
    for stream in [libc::STDOUT_FILENO, libc::STDERR_FILENO] {
        // unsafe(ffi)
        unsafe { libc::dup2(file.as_raw_fd(), stream) };
    }
    console
}

fn log_directory() -> Option<PathBuf> {
    let state = std::env::var_os("XDG_STATE_HOME")
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
        .or_else(|| {
            std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".local/state"))
        })?;
    Some(state.join(crate::app_id().to_lowercase()))
}
