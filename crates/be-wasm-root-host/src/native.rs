//! The root entry natively: the root is a `be_wasm_host::App` stepped on the calling thread, and
//! between steps the frame waits on a bell, up to the deadline and then up to the root's patience
//! past it.

use std::{
    collections::BTreeMap,
    sync::{Arc, Condvar, Mutex},
    time::{Duration, Instant},
};

use be_wasm_host::{App, PresentedTexture};
use be_wasm_root_api::{
    FrameDone, FrameRequest, host::LockState, host::Notice, host::PowerAction, host::StatisticRow,
};
use be_wasm_sandbox_host::{Loader, Server};

use crate::{Core, Inline, Password, Progress};

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

    /// The fonts the host bundles, which the root's `host.fonts` answers: be-sdk-api's `Fonts`
    /// in be-wasm-ops' encoding (`be_wasm_ops::encode`), as be-wasm crates use only be-wasm ones;
    /// none unless set.
    pub fn with_fonts(mut self, fonts: Vec<u8>) -> Self {
        self.core.fonts = fonts;
        self
    }

    /// Whether the host delivers the paste shortcut as a paste itself, as input
    /// (`host.paste-shortcut`): false unless set, as a native window's platform asks for pastes.
    pub fn with_paste_shortcut(mut self, delivers: bool) -> Self {
        self.core.paste_shortcut = delivers;
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

    /// Shows the person `shown`, the host's notices, oldest first, in place of those shown
    /// before: the root hears them from its `host.notices`, and if it waits on that, its bell
    /// rings, which wakes the host (`on_wake`) to step it.
    pub fn show_notices(&mut self, shown: Vec<Notice>) {
        if self.core.show_notices(shown) {
            self.bell.ring();
        }
    }

    /// Whether the root watches the host's statistics now (`host.watch-statistics`): while it
    /// does, the host measures them and shows them to it (`show_statistics`) as they change.
    pub fn watching_statistics(&self) -> bool {
        self.core.watching_statistics
    }

    /// Shows the root the host's statistics (`host.statistics`), in place of those shown before:
    /// its debug panels' rows of the host's frames and steps (`performance`) and memory
    /// (`memory`), and the file the memory statistics are written to. If the root waits on them
    /// and they changed, it is stepped for them.
    pub fn show_statistics(
        &mut self,
        performance: Vec<StatisticRow>,
        memory: Vec<StatisticRow>,
        memory_file: Option<String>,
    ) {
        let answered = self.core.show_statistics(performance, memory, memory_file);
        if answered {
            self.bell.ring();
        }
    }

    /// Shows the root whether the person's changes are saved (`host.saving`), in place of what
    /// it showed (its `generation` is ignored: the root counts the changes). If the root waits on
    /// it and it changed, it is stepped for it.
    pub fn show_saving(&mut self, shown: be_wasm_root_api::SavingState) {
        let answered = self.core.show_saving(shown);
        if answered {
            self.bell.ring();
        }
    }

    /// Shows the root what the block stack shows the person (`host.core`), its generation the
    /// host's to count. If the root waits on it and it changed, it is stepped for it.
    pub fn show_core(&mut self, shown: be_wasm_root_api::CoreView) {
        let answered = self.core.show_core(shown);
        if answered {
            self.bell.ring();
        }
    }

    /// Shows the root the host's memory over time (`host.memory-history`): the minutes'
    /// points, oldest first, and when the latest sample was taken, in seconds since the Unix
    /// epoch. If the root waits on it and it changed, it is stepped for it.
    pub fn show_memory_history(&mut self, points: Vec<be_wasm_root_api::MemoryPoint>, now: u64) {
        if self.core.show_memory_history(points, now) {
            self.bell.ring();
        }
    }

    /// What the root asked of the block stack since the last call (`host.ask-core`), oldest
    /// first, which the host does.
    pub fn take_core_commands(&mut self) -> Vec<be_wasm_root_api::CoreCommand> {
        self.core.take_core_commands()
    }

    /// What the root asked to make a secret since the last call (`host.make-secret`), oldest
    /// first, each by the id `answer_secret` answers it by.
    pub fn take_secret_requests(&mut self) -> Vec<(u64, be_wasm_root_api::SecretCommand)> {
        self.core.take_secret_requests()
    }

    /// Answers the request of `id` with the secret it made, handed over once, or with nothing
    /// if making it failed; one the root no longer waits for drops the secret. If the root
    /// waited for it, it is stepped for the answer. Answers whether it waited: a secret that
    /// cannot be had again and that nobody waited for, the host undoes.
    pub fn answer_secret(
        &mut self,
        id: u64,
        secret: Option<be_wasm_root_api::CoreSecret>,
    ) -> bool {
        let waited = self.core.answer_secret(id, secret);
        if waited {
            self.bell.ring();
        }
        waited
    }

    /// The blocks the root asked the editor of since the last call (`host.editor-of`), each by
    /// the id `answer_editor` answers it by.
    pub fn take_editor_asks(&mut self) -> Vec<(u64, [u8; 16])> {
        self.core.take_editor_asks()
    }

    /// Answers the root's `host.editor-of` of `id`. If the root waited for it, it is stepped for
    /// the answer.
    pub fn answer_editor(&mut self, id: u64, editor: Result<be_wasm_root_api::EditorApp, String>) {
        if self.core.answer_editor(id, editor) {
            self.bell.ring();
        }
    }

    /// The editors the root asked to open since the last call (`host.open-editor`), each
    /// answered by its id (`answer_opening`).
    pub fn take_openings(&mut self) -> Vec<crate::EditorOpening> {
        self.core.take_openings()
    }

    /// Answers the root's `host.open-editor` of `id`. If the root waited for it, it is stepped
    /// for the answer.
    pub fn answer_opening(
        &mut self,
        id: u64,
        opened: Result<be_wasm_root_api::OpenedEditor, String>,
    ) {
        if self.core.answer_opening(id, opened) {
            self.bell.ring();
        }
    }

    /// What the root forwarded of the apps it spawned since the last call (`host.forward`):
    /// each app's frames, by its instance, in order.
    pub fn take_forwarded(&mut self) -> Vec<(u64, Vec<Vec<u8>>)> {
        self.core.take_forwarded()
    }

    /// Sends the app the root spawned as `instance` `frames` through the root
    /// (`host.forwarded`). If the root waited for them, it is stepped for them.
    pub fn relay(&mut self, instance: u64, frames: Vec<Vec<u8>>) {
        if self.core.relay(instance, frames) {
            self.bell.ring();
        }
    }

    /// The staged app the root's sandbox instance `instance` runs, while it runs.
    pub fn app_of(&self, instance: u64) -> Option<String> {
        self.core.app_of(instance)
    }

    /// Shows the root whether the host asks the person to keep display settings that just
    /// changed (`host.displays`): the question, or none. If the root waits on it and it changed,
    /// it is stepped for it.
    pub fn show_displays(&mut self, asking: Option<be_wasm_root_api::DisplayQuestion>) {
        let answered = self.core.show_displays(asking);
        if answered {
            self.bell.ring();
        }
    }

    /// How the root answered the display questions since the last call (`host.answer-display`),
    /// oldest first: each round and whether it was kept, which the host does if it still asks
    /// that round.
    pub fn take_display_answers(&mut self) -> Vec<(u64, bool)> {
        self.core.take_display_answers()
    }

    /// Shows the root the session's screen lock (`host.lock`), in place of what it showed (its
    /// `generation` is ignored: the root counts the changes). If the root waits on it and it
    /// changed, it is stepped for it.
    pub fn show_lock(&mut self, shown: LockState) {
        let answered = self.core.show_lock(shown);
        if answered {
            self.bell.ring();
        }
    }

    /// Whether the root asked to lock the session (`host.lock-session`) since the last call,
    /// which the host does if it can.
    pub fn take_lock_asked(&mut self) -> bool {
        self.core.take_lock_asked()
    }

    /// The password the root handed over to unlock the session (`host.unlock`) since the last
    /// call, if it did: one at a time, only while the session is locked, nothing is being
    /// checked and the host makes no one wait (`retry-at`). The host checks it against the
    /// platform, drops it, which zeroes it, and shows the lock as it then stands
    /// (`show_lock`): unlocked, or refused with why, and the wait it imposes after a failure.
    /// That is all the root hears of it.
    pub fn take_unlock(&mut self) -> Option<Password> {
        self.core.take_unlock()
    }

    /// What the root chose on its lock screen besides unlocking (`host.lock-power`) since the
    /// last call, which the host does if the session is locked and offers it.
    pub fn take_lock_powers(&mut self) -> Vec<PowerAction> {
        self.core.take_lock_powers()
    }

    /// How the root answered the host's notices since the last call (`host.answer-notice`), in
    /// order: each notice's id and the key of the action the person chose, or none where they
    /// dismissed it. The host does what each asks.
    pub fn take_answers(&mut self) -> Vec<(u64, Option<String>)> {
        self.core.take_answers()
    }

    /// What the root copied since the last call (`host.copy`), oldest first: each copy's items,
    /// richest first, each be-lib-ui-types' `ClipItem` in be-wasm-ops' encoding. The host puts
    /// each on its clipboard in turn, so the last stays.
    pub fn take_copies(&mut self) -> Vec<Vec<Vec<u8>>> {
        self.core.take_copies()
    }

    /// The pastes the root asked for since the last call (`host.paste`), oldest first: each its
    /// id and the formats it takes, preferred first. The host reads its clipboard for each and
    /// answers it (`answer_paste`), in a later frame if its platform is slow to.
    pub fn take_pastes(&mut self) -> Vec<(u64, Vec<String>)> {
        self.core.take_pastes()
    }

    /// Answers the paste `id` with the first of its formats the host's clipboard held, a
    /// `ClipItem` in be-wasm-ops' encoding, or none: the root hears it as it steps, and if it
    /// waits on it, its bell rings, which wakes the host (`on_wake`) to step it.
    pub fn answer_paste(&mut self, id: u64, item: Option<Vec<u8>>) {
        if self.core.answer_paste(id, item) {
            self.bell.ring();
        }
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
