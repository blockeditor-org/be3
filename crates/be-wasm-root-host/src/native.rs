//! The root entry natively: the root is a `be_wasm_host::App` stepped on the calling thread, and
//! between steps the frame waits on a bell, up to the deadline and then up to the root's patience
//! past it.

use std::{
    collections::BTreeMap,
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
    /// What each of the root's own surfaces showed when it last presented it, kept until the
    /// root lets the surface go: one it configured afresh (a new size) shows its last frame
    /// until it presents again.
    own: BTreeMap<u32, PresentedTexture>,
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
    /// Also told of every ring: the host, which draws a frame for it (`Root::on_wake`).
    host: Mutex<Option<Wake>>,
}

/// What the host is told when the root has something to step for.
type Wake = Arc<dyn Fn() + Send + Sync>;

impl Bell {
    fn rung(&self) -> u64 {
        *self.rung.lock().unwrap_or_else(|held| held.into_inner())
    }

    fn ring(&self) {
        let mut rung = self.rung.lock().unwrap_or_else(|held| held.into_inner());
        *rung = rung.wrapping_add(1);
        self.ringing.notify_all();
        drop(rung);
        let host = self
            .host
            .lock()
            .unwrap_or_else(|held| held.into_inner())
            .clone();
        if let Some(wake) = host {
            wake();
        }
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
    /// by id are found by `apps` and draw on the host's `device`. A root that draws surfaces of
    /// its own is loaded on a host given that device too (`Host::with_display`).
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
            own: BTreeMap::new(),
        })
    }

    /// How long past a frame's deadline the root itself may take, `STEP_TIMEOUT` unless set.
    pub fn with_patience(mut self, patience: Duration) -> Self {
        self.core.patience = patience;
        self
    }

    /// The fonts the host bundles, which the root's `host.fonts` answers: none unless set.
    pub fn with_fonts(mut self, fonts: Vec<be_wasm_root_api::FontFace>) -> Self {
        self.core.fonts = fonts;
        self
    }

    /// Asks the root what its own surfaces painted, which it answers as it steps through the
    /// next frame (`take_paintings`). A root that stopped says why.
    pub fn ask_paintings(&mut self) -> Result<(), String> {
        self.core.ask_paintings()
    }

    /// What the root answered `ask_paintings` with, once it did: `be3:root`'s `paintings`,
    /// beui-paint-format's `ScreenPaintings` keyed by its own surfaces' ids, or no bytes.
    pub fn take_paintings(&mut self) -> Option<Vec<u8>> {
        self.core.take_paintings()
    }

    /// Calls `wake` on whatever thread it happens on whenever the root has something to step
    /// for between frames: an instance it spawned sent frames or stopped, a compile finished, or
    /// it woke itself. A host that draws only on demand draws a frame for it, which steps the
    /// root with what arrived; it is also called while a frame is drawn, which costs a host
    /// that draws on demand one more frame at most.
    pub fn on_wake(&self, wake: impl Fn() + Send + Sync + 'static) {
        *self
            .bell
            .host
            .lock()
            .unwrap_or_else(|held| held.into_inner()) = Some(Arc::new(wake));
    }

    /// Hands the root `window`'s input since the last call, in the window's points: `events` is
    /// be-lib-ui-types' `input::encode`, and `window` the layer list's `WindowId`, by its number
    /// (`None`: the host's one window). The root takes it in with the next `frame`, ahead of
    /// drawing it, and routes it by the layers it drew last. A root that stopped says why.
    pub fn input(&mut self, window: Option<u64>, events: Vec<u8>) -> Result<(), String> {
        self.core.input(window, events)
    }

    /// Draws a frame for `request`, waiting for the root's children until `deadline` and for the
    /// root itself as long as it takes, up to its patience past the deadline. A root that
    /// trapped, panicked or took longer says why, and draws nothing again.
    pub fn frame(&mut self, request: FrameRequest, deadline: Instant) -> Result<FrameDone, String> {
        self.core.begin(request, deadline)?;
        loop {
            let since = self.bell.rung();
            match self.core.advance(None) {
                Progress::Drawn(drawn) => {
                    self.presented();
                    return drawn;
                }
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

impl Root {
    /// What the root's own surface `surface` shows, as its layer list names it
    /// (be-lib-layers' `Surface::Root`): the texture it presented last to that surface of its
    /// gpu (`be3:gpu`'s `surface`), on the host's device, which the root drew on as an app
    /// draws, until it lets the surface go. One it configured afresh shows that texture still,
    /// until it presents again. None for a surface it never presented or let go of, or a root
    /// loaded with no device (`be_wasm_host::Host::with_display`).
    pub fn own_surface(&self, surface: u32) -> Option<&PresentedTexture> {
        self.own.get(&surface)
    }

    /// Takes what the root let go of and presented during the frame drawn, as the sandbox
    /// keeps what its instances present: the texture each showed when presented, even if the
    /// root configured it afresh later in the frame. A surface let go of and presented again is
    /// presented after, so it is kept.
    fn presented(&mut self) {
        let guest = &mut self.core.guest;
        for surface in guest.take_released() {
            self.own.remove(&surface);
        }
        for surface in guest.take_presented() {
            if let Some((texture, generation)) = guest.presented_surface(surface) {
                self.own.insert(
                    surface,
                    PresentedTexture {
                        surface,
                        texture,
                        generation,
                    },
                );
            }
        }
    }
}

impl Drop for Root {
    fn drop(&mut self) {
        self.core.guest.stop();
    }
}
