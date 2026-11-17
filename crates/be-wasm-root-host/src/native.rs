//! The root entry natively: the root is a `be_wasm_host::App` stepped on the calling thread, and
//! between steps the frame waits on a bell, up to the deadline and then up to the root's patience
//! past it.

use std::{
    sync::{Arc, Condvar, Mutex},
    time::{Duration, Instant},
};

use be_wasm_host::App;
use be_wasm_root_api::{FrameDone, FrameRequest};
use be_wasm_sandbox_host::{Loader, Server};

use crate::{Core, Inline, Progress};

/// The root guest, run inline on the thread that holds it. Dropping it stops the root and every
/// instance it spawned.
pub struct Root {
    core: Core<App>,
    bell: Arc<Bell>,
}

impl Inline for App {
    fn deliver(&mut self, frames: Vec<Vec<u8>>) {
        App::deliver(self, frames);
    }

    fn step(&mut self) -> Result<(), String> {
        App::step(self)
    }

    fn take_outbound(&mut self) -> Vec<Vec<u8>> {
        App::take_outbound(self)
    }

    fn take_wake(&mut self) -> bool {
        App::take_wake(self)
    }
}

/// Rung whenever something the root waits on may have arrived: an instance it spawned sent
/// frames or stopped, a compile finished, or the root woke itself.
#[derive(Default)]
struct Bell {
    rung: Mutex<u64>,
    ringing: Condvar,
}

impl Bell {
    fn rung(&self) -> u64 {
        *self.rung.lock().unwrap_or_else(|held| held.into_inner())
    }

    fn ring(&self) {
        let mut rung = self.rung.lock().unwrap_or_else(|held| held.into_inner());
        *rung = rung.wrapping_add(1);
        self.ringing.notify_all();
    }

    /// Waits until it rings after `since` or `until` passes.
    fn wait(&self, since: u64, until: Instant) {
        let rung = self.rung.lock().unwrap_or_else(|held| held.into_inner());
        let left = until.saturating_duration_since(Instant::now());
        drop(
            self.ringing
                .wait_timeout_while(rung, left, |rung| *rung == since)
                .unwrap_or_else(|held| held.into_inner()),
        );
    }
}

impl Root {
    /// Starts `app`, the root guest, loaded on the calling thread, whose staged apps it spawns
    /// by id are found by `apps`.
    pub fn start(mut app: App, apps: Loader) -> Result<Self, String> {
        let bell = Arc::new(Bell::default());
        let ringer = Arc::clone(&bell);
        app.on_wake(move || ringer.ring());
        let ringer = Arc::clone(&bell);
        let sandbox = Server::new(Arc::new(move || ringer.ring()), apps);
        app.start()?;
        Ok(Self {
            core: Core::new(app, sandbox)?,
            bell,
        })
    }

    /// How long past a frame's deadline the root itself may take, `STEP_TIMEOUT` unless set.
    pub fn with_patience(mut self, patience: Duration) -> Self {
        self.core.patience = patience;
        self
    }

    /// Draws a frame for `request`, waiting for the root's children until `deadline` and for the
    /// root itself as long as it takes, up to its patience past the deadline. A root that
    /// trapped, panicked or took longer says why, and draws nothing again.
    pub fn frame(&mut self, request: FrameRequest, deadline: Instant) -> Result<FrameDone, String> {
        self.core.begin(request, deadline)?;
        loop {
            let since = self.bell.rung();
            match self.core.advance() {
                Progress::Drawn(drawn) => return drawn,
                // Pending: wait until what the root waits on has something for it.
                Progress::Pending { until } => self.bell.wait(since, until),
            }
        }
    }
}

impl Drop for Root {
    fn drop(&mut self) {
        self.core.guest.stop();
    }
}
