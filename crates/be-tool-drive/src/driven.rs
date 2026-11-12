use std::path::{Path, PathBuf};
use std::process::Child;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;

use beui_layer_drive::Reply;

use crate::{Unanswered, launch, request};

/// How beui-layer-drive's `close` answers once the window closed.
const CLOSED: &str = "The window closed.";
const CLOSING: Duration = Duration::from_secs(60);
const LOG_TAIL: usize = 40;
const PANIC_LINES: usize = 12;

/// What [`Driven::start`] starts.
#[derive(Clone, Debug, Default)]
pub struct Start {
    pub program: String,
    pub arguments: Vec<String>,
    /// The headless window's size, `WIDTHxHEIGHT[@SCALE]`.
    pub size: String,
    /// `LD_LIBRARY_PATH` for the program.
    pub libraries: Option<String>,
    pub space: bool,
    pub render: bool,
    /// Set in the program's environment on top of what `be-drive run` sets.
    pub environment: Vec<(String, String)>,
}

/// A program started headless for a test, with fresh data in a directory of its own and its
/// clock pinned, as `be-drive run` starts one: crates/be-host-tests drives be-host through it.
///
/// Unless `render` is set it runs with `BEUI_RENDERER=none`: frames are painted but no GPU draws
/// them, so `paint` works and `shot` does not, and neither does anything that is a texture
/// (space's 3D view, block previews).
pub struct Driven {
    child: Option<Child>,
    dir: PathBuf,
    socket: PathBuf,
    log: PathBuf,
    timeout: Duration,
    start: Start,
    starts: u32,
}

impl Driven {
    pub fn start(start: Start) -> Result<Self, String> {
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
        let child = launch::start(&start, true, &dir, &log)?;
        Ok(Self {
            child: Some(child),
            socket: dir.join("drive.sock"),
            dir,
            log,
            timeout: crate::TIMEOUT,
            start,
            starts: 1,
        })
    }

    /// Closes the program as [`Driven::close`] does, failing unless it exits cleanly, and
    /// starts it again on the data it kept, as a person quitting and opening it again. The log
    /// starts over in a file of its own, so the one before stays beside it.
    pub fn restart(&mut self) -> Result<(), String> {
        self.close_child()?;
        self.starts += 1;
        self.log = self.dir.join(format!("app.{}.log", self.starts));
        self.child = Some(launch::start(&self.start, false, &self.dir, &self.log)?);
        Ok(())
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

    /// Asks the program to close and fails unless it exits cleanly within [`CLOSING`].
    pub fn close(mut self) -> Result<(), String> {
        self.close_child()
    }

    fn close_child(&mut self) -> Result<(), String> {
        let Some(child) = self.child.take() else {
            return Ok(());
        };
        let closed = match request(&self.socket, &["close".to_owned()], self.timeout) {
            Ok(Reply::Text(text)) if !text.starts_with(CLOSED) => {
                launch::kill(child.id());
                Err(format!(
                    "the program stayed open when asked to close (to ask about unsaved changes?), and changed:\n{text}"
                ))
            }
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
    let mut panics: Vec<(usize, usize)> = Vec::new();
    for (start, line) in lines.iter().enumerate() {
        if line.contains(" panicked") && panics.last().is_none_or(|(_, end)| *end <= start) {
            panics.push((start, panic_end(&lines, start)));
        }
    }
    let mut tail = lines.len().saturating_sub(LOG_TAIL);
    if let Some(&(start, _)) = panics
        .iter()
        .find(|(start, end)| *start < tail && tail < *end)
    {
        tail = start;
    }
    let before: Vec<String> = panics
        .iter()
        .filter(|(start, _)| *start < tail)
        .map(|&(start, end)| lines[start..end.min(tail)].join("\n"))
        .collect();
    let end = format!("The end of the app's log:\n{}", lines[tail..].join("\n"));
    match before.is_empty() {
        true => end,
        false => format!(
            "The app's panics before that:\n{}\n\n{end}",
            before.join("\n\n")
        ),
    }
}

fn panic_end(lines: &[&str], start: usize) -> usize {
    let message = lines[start + 1..]
        .iter()
        .take(PANIC_LINES)
        .position(|line| {
            line.is_empty() || line.starts_with("note: run with") || *line == "stack backtrace:"
        })
        .map_or_else(
            || lines.len().min(start + 1 + PANIC_LINES),
            |end| start + 1 + end,
        );
    let mut end = message;
    while end < lines.len() && (lines[end].is_empty() || lines[end] == "stack backtrace:") {
        end += 1;
    }
    let frames = lines[end..]
        .iter()
        .take_while(|line| backtrace_line(line))
        .count();
    if frames == 0 {
        return message;
    }
    end += frames;
    if lines
        .get(end)
        .is_some_and(|line| line.starts_with("note: Some details are omitted"))
    {
        end += 1;
    }
    end
}

fn backtrace_line(line: &str) -> bool {
    let trimmed = line.trim_start();
    if trimmed.len() == line.len() {
        return false;
    }
    trimmed.starts_with("at ")
        || trimmed.split_once(": ").is_some_and(|(frame, _)| {
            !frame.is_empty() && frame.bytes().all(|b| b.is_ascii_digit())
        })
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
