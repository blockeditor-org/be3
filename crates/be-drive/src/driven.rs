use std::path::{Path, PathBuf};
use std::process::Child;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;

use beui_layer_drive::Reply;

use crate::{Unanswered, launch, request};

const CLOSING: Duration = Duration::from_secs(60);
const LOG_TAIL: usize = 40;
const PANIC_LINES: usize = 12;

pub struct Driven {
    child: Option<Child>,
    dir: PathBuf,
    socket: PathBuf,
    log: PathBuf,
    timeout: Duration,
}

impl Driven {
    pub fn start(
        program: &str,
        arguments: &[&str],
        size: &str,
        libraries: Option<String>,
    ) -> Result<Self, String> {
        static STARTED: AtomicU32 = AtomicU32::new(0);
        let dir = Path::new("/tmp").join(format!(
            "be3-{}-{}",
            std::process::id(),
            STARTED.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir)
            .map_err(|error| format!("could not create {}: {error}", dir.display()))?;
        let log = dir.join("app.log");
        let child = launch::start(
            program,
            arguments
                .iter()
                .map(|argument| (*argument).to_owned())
                .collect(),
            size,
            libraries,
            &dir,
            &log,
        )?;
        Ok(Self {
            child: Some(child),
            socket: dir.join("drive.sock"),
            dir,
            log,
            timeout: crate::TIMEOUT,
        })
    }

    pub fn set_timeout(&mut self, timeout: Duration) {
        self.timeout = timeout;
    }

    pub fn log_report(&self) -> String {
        log_report(&std::fs::read_to_string(&self.log).unwrap_or_default())
    }

    pub fn request(&self, words: &[&str]) -> Result<Reply, String> {
        let words: Vec<String> = words.iter().map(|word| (*word).to_owned()).collect();
        request(&self.socket, &words, self.timeout).map_err(|error| error.to_string())
    }

    pub fn close(mut self) -> Result<(), String> {
        let Some(child) = self.child.take() else {
            return Ok(());
        };
        let closed = match request(&self.socket, &["close".to_owned()], self.timeout) {
            Ok(_) | Err(Unanswered::Closed) => launch::wait_for_exit(child, CLOSING),
            Err(Unanswered::Failed(error)) => {
                launch::kill(child.id());
                Err(error)
            }
        };
        closed.map_err(|error| format!("{error}\n\n{}", self.log_report()))
    }
}

pub fn log_report(log: &str) -> String {
    let lines: Vec<&str> = log.lines().collect();
    let tail = lines.len().saturating_sub(LOG_TAIL);
    let panics: Vec<String> = lines[..tail]
        .iter()
        .enumerate()
        .filter(|(_, line)| line.contains(" panicked"))
        .map(|(start, _)| {
            let end = lines[start + 1..tail]
                .iter()
                .take(PANIC_LINES)
                .position(|line| {
                    line.is_empty()
                        || line.starts_with("note: run with")
                        || line.starts_with("stack backtrace:")
                })
                .map_or_else(|| tail.min(start + 1 + PANIC_LINES), |end| start + 1 + end);
            lines[start..end].join("\n")
        })
        .collect();
    let tail = format!("The end of the app's log:\n{}", lines[tail..].join("\n"));
    match panics.is_empty() {
        true => tail,
        false => format!(
            "The app's panics before that:\n{}\n\n{tail}",
            panics.join("\n\n")
        ),
    }
}

impl Drop for Driven {
    fn drop(&mut self) {
        if let Some(mut child) = self.child.take() {
            launch::kill(child.id());
            let _ = child.wait();
        }
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

#[cfg(test)]
mod tests;
