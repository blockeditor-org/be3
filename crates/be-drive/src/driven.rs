use std::path::{Path, PathBuf};
use std::process::Child;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;

use beui_core::app::automation::Reply;

use crate::{Unanswered, launch, request};

const CLOSING: Duration = Duration::from_secs(60);

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
            socket: dir.join("automation.sock"),
            dir,
            log,
            timeout: crate::TIMEOUT,
        })
    }

    pub fn log(&self) -> &Path {
        &self.log
    }

    pub fn request(&self, words: &[&str]) -> Result<Reply, String> {
        let words: Vec<String> = words.iter().map(|word| (*word).to_owned()).collect();
        request(&self.socket, &words, self.timeout).map_err(|error| error.to_string())
    }

    pub fn close(mut self) -> Result<(), String> {
        let Some(child) = self.child.take() else {
            return Ok(());
        };
        match request(&self.socket, &["close".to_owned()], self.timeout) {
            Ok(_) | Err(Unanswered::Closed) => {}
            Err(Unanswered::Failed(error)) => {
                launch::kill(child.id());
                return Err(error);
            }
        }
        launch::wait_for_exit(child, CLOSING)
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
