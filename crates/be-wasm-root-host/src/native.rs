//! The root entry natively: the root is a `be_wasm_host::App` stepped on the calling thread, and
//! between steps the frame waits on a bell, up to the deadline and then up to the root's patience
//! past it.

use std::{
    sync::{Arc, Condvar, Mutex},
    time::{Duration, Instant},
};

use be_wasm_host::{App, PresentedTexture};
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
    /// by id are found by `apps` and draw on the host's `device`.
    pub fn start(
        mut app: App,
        apps: Loader,
        device: wgpu::Device,
        queue: wgpu::Queue,
    ) -> Result<Self, String> {
        let bell = Arc::new(Bell::default());
        let ringer = Arc::clone(&bell);
        app.on_wake(move || ringer.ring());
        let ringer = Arc::clone(&bell);
        let sandbox =
            Server::new(Arc::new(move || ringer.ring()), apps).with_display(device, queue);
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
            match self.core.advance(None) {
                Progress::Drawn(drawn) => return drawn,
                // Pending: wait until what the root waits on has something for it.
                Progress::Pending { until } => self.bell.wait(since, until),
                // Natively the frame thread is the root's alone: it never yields.
                Progress::Yielded => {}
            }
        }
    }

    /// What the surface `surface` of the instance the root spawned with the op `instance` shows,
    /// as the root's layer list names it (be-lib-layers' `Surface::Screen` or `Anchored`, whose
    /// `instance` is that instance's `instance` resource's op): the texture it presented last to
    /// that surface, on the host's device. The root never sees it: it names it only by handle,
    /// and only the host draws it. None for a surface never presented.
    ///
    /// ```ignore
    /// // The root, which spawned `child` (a `be3:sandbox` instance), lists its screen:
    /// let surface = Surface::Screen { instance: child.op(), surface: 7 };
    /// // The host, given the root's frame, finds what that names:
    /// if let Surface::Screen { instance, surface } = layer.surface {
    ///     let texture = root.surface(instance, surface).map(|shown| &shown.texture);
    /// }
    /// ```
    ///
    /// The tests' root does exactly this (be-wasm-root-testing's `PAINTS`).
    pub fn surface(&self, instance: u64, surface: u32) -> Option<&PresentedTexture> {
        self.core.sandbox.surface(instance, surface)
    }
}

impl Drop for Root {
    fn drop(&mut self) {
        self.core.guest.stop();
    }
}
