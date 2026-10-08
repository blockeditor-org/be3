use std::fs::{OpenOptions, Permissions};
use std::io::Write;
use std::os::fd::AsRawFd;
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

const LOG: &str = "session.log";
const PREVIOUS_LOG: &str = "session.previous.log";

pub(crate) fn start_log() {
    let Some(directory) = log_directory() else {
        eprintln!("block-app: no directory for the session log; set XDG_STATE_HOME or HOME");
        return;
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
                "block-app: the session log {} did not open: {error}",
                path.display()
            );
            return;
        }
    };
    let started = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let _ = writeln!(
        file,
        "Block {} started a session at {started} seconds after the Unix epoch.",
        crate::COMMIT
    );
    eprintln!("block-app: the session logs to {}", path.display());
    for stream in [libc::STDOUT_FILENO, libc::STDERR_FILENO] {
        unsafe { libc::dup2(file.as_raw_fd(), stream) };
    }
}

fn log_directory() -> Option<PathBuf> {
    let state = std::env::var_os("XDG_STATE_HOME")
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
        .or_else(|| {
            std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".local/state"))
        })?;
    Some(state.join(crate::APP_ID.to_lowercase()))
}
