//! The root entry in the browser (plans/host_as_platform_adapter.md, The root guest): the root
//! is instantiated on the page and stepped inline on its thread, where WebXR and
//! `requestAnimationFrame` run and from which no worker can be called synchronously. The page
//! never waits (decisions/the_web_page_never_waits.md), so a frame is not one call as natively:
//! `Root::frame`, called from `requestAnimationFrame`, steps the root as far as it goes, for a
//! slice of the thread at most (`SLICE`), and returns, and whatever the root waits on resumes it
//! later: an instance's worker answering, a compile finishing, the root waking itself, a timer
//! at the frame's deadline and at its patience past it, or, once its slice ran out, a task of its
//! own after the page's others. The frame is handed over once the root answers.

use std::{
    cell::{Cell, RefCell},
    collections::{HashMap, VecDeque},
    rc::{Rc, Weak},
    sync::Arc,
    time::{Duration, Instant},
};

use be_wasm_guest_host::Wake;
use be_wasm_root_api::{
    FrameDone, FrameRequest, host::LockState, host::Notice, host::PowerAction, host::StatisticRow,
};
use be_wasm_sandbox_host::{Loader, Server};
use js_sys::{Function, Object, Reflect, Uint8Array, WebAssembly};
use wasm_bindgen::{JsCast, prelude::*};
use wasm_bindgen_futures::JsFuture;

use crate::{Core, Inline, Password, Progress};

/// What hears a drawn frame, or why the root drew none.
type Drawn = Box<dyn FnOnce(Result<FrameDone, String>)>;

/// The root guest, run inline on the page. Dropping it stops the root and every instance it
/// spawned.
pub struct Root {
    shared: Rc<Shared>,
}

struct Shared {
    id: u64,
    core: RefCell<Core<PageGuest>>,
    /// What hears the frame being drawn.
    drawn: RefCell<Option<Drawn>>,
    /// Whether a task to resume the root is queued already.
    queued: Cell<bool>,
    /// The timer that resumes the root at the moment it waits for: the frame's deadline, or its
    /// patience past it.
    timer: RefCell<Option<Timer>>,
    /// Whether the root is being stepped, during which what rings is the step's to take.
    stepping: Cell<bool>,
    /// Resumes a root that used its slice of the page's thread, as a task of its own, after
    /// whatever input and rendering the page has waiting.
    after: web_sys::MessageChannel,
    _resumed: Closure<dyn FnMut(web_sys::MessageEvent)>,
}

/// The most of the page's thread a root steps for at once: one step at least, and others while
/// they fit. A root with more to do resumes after a task's turn, so a root busy for long, or one
/// stuck waking itself, never holds up the page's input and rendering.
const SLICE: Duration = Duration::from_millis(4);

thread_local! {
    /// Every root on the page by its number, which its wakes name since they may not hold it:
    /// a wake is `Send`, and the page's values are not.
    static ROOTS: RefCell<HashMap<u64, Weak<Shared>>> = RefCell::new(HashMap::new());
    static NEXT: Cell<u64> = const { Cell::new(0) };
}

fn root(id: u64) -> Option<Rc<Shared>> {
    ROOTS.with(|roots| roots.borrow().get(&id).and_then(Weak::upgrade))
}

/// Something the root `id` waits on may have arrived: queues a microtask that resumes its frame.
/// This is the first wake after the root waited, so the root takes it before anything else; a
/// root that keeps stepping resumes as a task of its own instead (`SLICE`). What rings while the
/// root is stepped is the step's to take, from its own state.
fn ring(id: u64) {
    let Some(shared) = root(id) else {
        return;
    };
    if shared.stepping.get() || shared.queued.replace(true) {
        return;
    }
    let held = Rc::downgrade(&shared);
    let resume = Closure::once_into_js(move || {
        if let Some(shared) = held.upgrade() {
            shared.queued.set(false);
            shared.resume();
        }
    });
    // Queued, never run inline: a wake may come from inside a call into the root's host.
    let global = js_sys::global();
    if let Ok(queue) = Reflect::get(&global, &"queueMicrotask".into())
        && let Ok(queue) = queue.dyn_into::<Function>()
    {
        let _ = queue.call1(&global, &resume);
    }
}

impl Root {
    /// Fetches the root guest from `url`, compiles it with `WebAssembly.compile` and starts it on
    /// the page. The staged apps it spawns by id are found by `apps`.
    pub async fn load(url: &str, apps: Loader) -> Result<Self, String> {
        let window = web_sys::window().ok_or("the root runs only on a page")?;
        let fetched = JsFuture::from(window.fetch_with_str(url))
            .await
            .map_err(|error| format!("{url} could not be fetched: {}", describe(&error)))?;
        let response: web_sys::Response = fetched.unchecked_into();
        if !response.ok() {
            return Err(format!("{url} could not be fetched: {}", response.status()));
        }
        let bytes = response
            .array_buffer()
            .map_err(|error| format!("{url} could not be read: {}", describe(&error)))?;
        let bytes = JsFuture::from(bytes)
            .await
            .map_err(|error| format!("{url} could not be read: {}", describe(&error)))?;
        let module = JsFuture::from(WebAssembly::compile(&bytes))
            .await
            .map_err(|error| format!("{url} is not a valid module: {}", describe(&error)))?;
        Self::start(&module.unchecked_into(), apps).await
    }

    /// Starts an instance of `module`, the root guest, on the page.
    pub async fn start(module: &WebAssembly::Module, apps: Loader) -> Result<Self, String> {
        let id = NEXT.with(|next| next.replace(next.get() + 1));
        let wake: Wake = Arc::new(move || ring(id));
        let guest = PageGuest::start(module, Arc::clone(&wake)).await?;
        let core = Core::new(guest, Server::new(wake, apps))?;
        let after = web_sys::MessageChannel::new().map_err(|error| {
            format!(
                "the root's page has no MessageChannel: {}",
                describe(&error)
            )
        })?;
        let resumed = Closure::<dyn FnMut(web_sys::MessageEvent)>::new(move |_| {
            if let Some(shared) = root(id) {
                shared.resume();
            }
        });
        after
            .port1()
            .set_onmessage(Some(resumed.as_ref().unchecked_ref()));
        let shared = Rc::new(Shared {
            id,
            core: RefCell::new(core),
            drawn: RefCell::new(None),
            queued: Cell::new(false),
            timer: RefCell::new(None),
            stepping: Cell::new(false),
            after,
            _resumed: resumed,
        });
        ROOTS.with(|roots| roots.borrow_mut().insert(id, Rc::downgrade(&shared)));
        Ok(Self { shared })
    }

    /// How long past a frame's deadline the root itself may take, `STEP_TIMEOUT` unless set.
    pub fn with_patience(self, patience: Duration) -> Self {
        self.shared.core.borrow_mut().patience = patience;
        self
    }

    /// The fonts the host bundles, which the root's `host.fonts` answers: be-sdk-api's `Fonts`
    /// in be-wasm-ops' encoding (`be_wasm_ops::encode`), as be-wasm crates use only be-wasm ones;
    /// none unless set.
    pub fn with_fonts(self, fonts: Vec<u8>) -> Self {
        self.shared.core.borrow_mut().fonts = fonts;
        self
    }

    /// Whether the page delivers the paste shortcut as a paste itself, as input, from the
    /// browser's paste event (`host.paste-shortcut`), which a page does, so every page that
    /// runs a root sets it (be-wasm-root-web-testing's does): false unless set.
    pub fn with_paste_shortcut(self, delivers: bool) -> Self {
        self.shared.core.borrow_mut().paste_shortcut = delivers;
        self
    }

    /// Asks the root what its own surfaces painted, which it answers as it steps through the
    /// next frame (`take_paintings`). A root that stopped says why.
    pub fn ask_paintings(&self) -> Result<(), String> {
        self.shared.core.borrow_mut().ask_paintings()
    }

    /// What the root answered `ask_paintings` with, once it did: be3:root's `paintings`.
    pub fn take_paintings(&self) -> Option<Vec<u8>> {
        self.shared.core.borrow_mut().take_paintings()
    }

    /// Shows the person `shown`, the host's notices, oldest first, in place of those shown
    /// before: the root hears them from its `host.notices`, stepped for them at once if it waits
    /// on that.
    pub fn show_notices(&self, shown: Vec<Notice>) {
        let answered = self.shared.core.borrow_mut().show_notices(shown);
        if answered {
            ring(self.shared.id);
        }
    }

    /// Whether the root watches the host's statistics now (`host.watch-statistics`): while it
    /// does, the host measures them and shows them to it (`show_statistics`) as they change.
    pub fn watching_statistics(&self) -> bool {
        self.shared.core.borrow().watching_statistics
    }

    /// Shows the root the host's statistics (`host.statistics`), in place of those shown before:
    /// its debug panels' rows of the host's frames and steps (`performance`) and memory
    /// (`memory`), and the file the memory statistics are written to. If the root waits on them
    /// and they changed, it is stepped for them.
    pub fn show_statistics(
        &self,
        performance: Vec<StatisticRow>,
        memory: Vec<StatisticRow>,
        memory_file: Option<String>,
    ) {
        let answered =
            self.shared
                .core
                .borrow_mut()
                .show_statistics(performance, memory, memory_file);
        if answered {
            ring(self.shared.id);
        }
    }

    /// Shows the root whether the person's changes are saved (`host.saving`), in place of what
    /// it showed (its `generation` is ignored: the root counts the changes). If the root waits on
    /// it and it changed, it is stepped for it.
    pub fn show_saving(&self, shown: be_wasm_root_api::SavingState) {
        let answered = self.shared.core.borrow_mut().show_saving(shown);
        if answered {
            ring(self.shared.id);
        }
    }

    /// Shows the root what the block stack shows the person (`host.core`), its generation the
    /// host's to count. If the root waits on it and it changed, it is stepped for it.
    pub fn show_core(&self, shown: be_wasm_root_api::CoreView) {
        let answered = self.shared.core.borrow_mut().show_core(shown);
        if answered {
            ring(self.shared.id);
        }
    }

    /// Shows the root the host's memory over time (`host.memory-history`): the minutes'
    /// points, oldest first, and when the latest sample was taken, in seconds since the Unix
    /// epoch. If the root waits on it and it changed, it is stepped for it.
    pub fn show_memory_history(&self, points: Vec<be_wasm_root_api::MemoryPoint>, now: u64) {
        if self
            .shared
            .core
            .borrow_mut()
            .show_memory_history(points, now)
        {
            ring(self.shared.id);
        }
    }

    /// What the root asked of the block stack since the last call (`host.ask-core`), oldest
    /// first, which the host does.
    pub fn take_core_commands(&self) -> Vec<be_wasm_root_api::CoreCommand> {
        self.shared.core.borrow_mut().take_core_commands()
    }

    /// What the root asked to make a secret since the last call (`host.make-secret`), oldest
    /// first, each by the id `answer_secret` answers it by.
    pub fn take_secret_requests(&self) -> Vec<(u64, be_wasm_root_api::SecretCommand)> {
        self.shared.core.borrow_mut().take_secret_requests()
    }

    /// Answers the request of `id` with the secret it made, handed over once, or with nothing
    /// if making it failed; one the root no longer waits for drops the secret. If the root
    /// waited for it, it is stepped for the answer. Answers whether it waited: a secret that
    /// cannot be had again and that nobody waited for, the host undoes.
    pub fn answer_secret(&self, id: u64, secret: Option<be_wasm_root_api::CoreSecret>) -> bool {
        let waited = self.shared.core.borrow_mut().answer_secret(id, secret);
        if waited {
            ring(self.shared.id);
        }
        waited
    }

    /// The blocks the root asked the editor of since the last call (`host.editor-of`), each by
    /// the id `answer_editor` answers it by.
    pub fn take_editor_asks(&self) -> Vec<(u64, [u8; 16])> {
        self.shared.core.borrow_mut().take_editor_asks()
    }

    /// Answers the root's `host.editor-of` of `id`. If the root waited for it, it is stepped for
    /// the answer.
    pub fn answer_editor(&self, id: u64, editor: Result<be_wasm_root_api::EditorApp, String>) {
        if self.shared.core.borrow_mut().answer_editor(id, editor) {
            ring(self.shared.id);
        }
    }

    /// The editors the root asked to open since the last call (`host.open-editor`), each
    /// answered by its id (`answer_opening`).
    pub fn take_openings(&self) -> Vec<crate::EditorOpening> {
        self.shared.core.borrow_mut().take_openings()
    }

    /// Answers the root's `host.open-editor` of `id`. If the root waited for it, it is stepped
    /// for the answer.
    pub fn answer_opening(&self, id: u64, opened: Result<be_wasm_root_api::OpenedEditor, String>) {
        if self.shared.core.borrow_mut().answer_opening(id, opened) {
            ring(self.shared.id);
        }
    }

    /// What the root forwarded of the apps it spawned since the last call (`host.forward`):
    /// each app's frames, by its instance, in order.
    pub fn take_forwarded(&self) -> Vec<(u64, Vec<Vec<u8>>)> {
        self.shared.core.borrow_mut().take_forwarded()
    }

    /// Sends the app the root spawned as `instance` `frames` through the root
    /// (`host.forwarded`). If the root waited for them, it is stepped for them.
    pub fn relay(&self, instance: u64, frames: Vec<Vec<u8>>) {
        if self.shared.core.borrow_mut().relay(instance, frames) {
            ring(self.shared.id);
        }
    }

    /// The staged app the root's sandbox instance `instance` runs, while it runs.
    pub fn app_of(&self, instance: u64) -> Option<String> {
        self.shared.core.borrow().app_of(instance)
    }

    /// Shows the root whether the host asks the person to keep display settings that just
    /// changed (`host.displays`): the question, or none. If the root waits on it and it changed,
    /// it is stepped for it.
    pub fn show_displays(&self, asking: Option<be_wasm_root_api::DisplayQuestion>) {
        let answered = self.shared.core.borrow_mut().show_displays(asking);
        if answered {
            ring(self.shared.id);
        }
    }

    /// How the root answered the display questions since the last call (`host.answer-display`),
    /// oldest first: each round and whether it was kept, which the host does if it still asks
    /// that round.
    pub fn take_display_answers(&self) -> Vec<(u64, bool)> {
        self.shared.core.borrow_mut().take_display_answers()
    }

    /// Shows the root the session's screen lock (`host.lock`), in place of what it showed (its
    /// `generation` is ignored: the root counts the changes). If the root waits on it and it
    /// changed, it is stepped for it.
    pub fn show_lock(&self, shown: LockState) {
        let answered = self.shared.core.borrow_mut().show_lock(shown);
        if answered {
            ring(self.shared.id);
        }
    }

    /// Whether the root asked to lock the session (`host.lock-session`) since the last call,
    /// which the host does if it can.
    pub fn take_lock_asked(&self) -> bool {
        self.shared.core.borrow_mut().take_lock_asked()
    }

    /// The password the root handed over to unlock the session (`host.unlock`) since the last
    /// call, if it did: one at a time, only while the session is locked, nothing is being
    /// checked and the host makes no one wait (`retry-at`). The host checks it against the
    /// platform, drops it, which zeroes it, and shows the lock as it then stands
    /// (`show_lock`): unlocked, or refused with why, and the wait it imposes after a failure.
    /// That is all the root hears of it.
    pub fn take_unlock(&self) -> Option<Password> {
        self.shared.core.borrow_mut().take_unlock()
    }

    /// What the root chose on its lock screen besides unlocking (`host.lock-power`) since the
    /// last call, which the host does if the session is locked and offers it.
    pub fn take_lock_powers(&self) -> Vec<PowerAction> {
        self.shared.core.borrow_mut().take_lock_powers()
    }

    /// How the root answered the host's notices since the last call (`host.answer-notice`), in
    /// order: each notice's id and the key of the action the person chose, or none where they
    /// dismissed it. The host does what each asks.
    pub fn take_answers(&self) -> Vec<(u64, Option<String>)> {
        self.shared.core.borrow_mut().take_answers()
    }

    /// What the root copied since the last call (`host.copy`), oldest first: each copy's items,
    /// richest first, each be-lib-ui-types' `ClipItem` in be-wasm-ops' encoding.
    pub fn take_copies(&self) -> Vec<Vec<Vec<u8>>> {
        self.shared.core.borrow_mut().take_copies()
    }

    /// The pastes the root asked for since the last call (`host.paste`), oldest first: each its
    /// id and the formats it takes, preferred first, which the page answers (`answer_paste`).
    pub fn take_pastes(&self) -> Vec<(u64, Vec<String>)> {
        self.shared.core.borrow_mut().take_pastes()
    }

    /// Answers the paste `id` with the first of its formats the page's clipboard held, a
    /// `ClipItem` in be-wasm-ops' encoding, or none: the root is stepped for it at once if it
    /// waits on it.
    pub fn answer_paste(&self, id: u64, item: Option<Vec<u8>>) {
        let answered = self.shared.core.borrow_mut().answer_paste(id, item);
        if answered {
            ring(self.shared.id);
        }
    }

    /// Hands the root `window`'s input since the last call, in the window's points: `events` is
    /// be-lib-ui-types' `input::encode`, and `window` the layer list's `WindowId`, by its number
    /// (`None`: the page's one window). The root takes it in with the frame being drawn, on its
    /// next resume, or else with the next `frame`, and routes it by the layers it drew last. A
    /// root that stopped says why.
    pub fn input(&self, window: Option<u64>, events: Vec<u8>) -> Result<(), String> {
        self.shared.core.borrow_mut().input(window, events)
    }

    /// Starts drawing a frame for `request`, called from `requestAnimationFrame`: steps the root
    /// inline as far as it goes, and resumes it from a task whenever something it waits on
    /// arrives, waiting for its children until `deadline` and for the root itself up to its
    /// patience past it. `drawn` hears the frame once the root answers, before this returns if
    /// it answered inline; a root that trapped, panicked or took longer says why, and draws
    /// nothing again. Answers false, and starts nothing, while an earlier frame is still being
    /// drawn.
    pub fn frame(
        &self,
        request: FrameRequest,
        deadline: Instant,
        drawn: impl FnOnce(Result<FrameDone, String>) + 'static,
    ) -> bool {
        let progress = {
            let mut core = self.shared.core.borrow_mut();
            if core.drawing() {
                return false;
            }
            match core.begin(request, deadline) {
                Ok(()) => {
                    *self.shared.drawn.borrow_mut() = Some(Box::new(drawn));
                    self.shared.stepping.set(true);
                    let progress = core.advance(Some(SLICE));
                    self.shared.stepping.set(false);
                    progress
                }
                Err(failure) => {
                    drop(core);
                    drawn(Err(failure));
                    return true;
                }
            }
        };
        self.shared.settle(progress);
        true
    }

    /// Whether a frame is being drawn: the root waits for its children, or for its deadline.
    pub fn drawing(&self) -> bool {
        self.shared.core.borrow().drawing()
    }
}

impl Shared {
    /// Steps the frame being drawn on from where it waited, if one is.
    fn resume(&self) {
        let progress = {
            let mut core = self.core.borrow_mut();
            if !core.drawing() {
                return;
            }
            self.stepping.set(true);
            let progress = core.advance(Some(SLICE));
            self.stepping.set(false);
            progress
        };
        self.settle(progress);
    }

    /// Hands a drawn frame over, or sets the timer for the moment a pending one waits for.
    fn settle(&self, progress: Progress) {
        self.timer.borrow_mut().take();
        match progress {
            Progress::Drawn(drawn) => {
                let hears = self.drawn.borrow_mut().take();
                if let Some(hears) = hears {
                    hears(drawn);
                }
            }
            Progress::Pending { until } => {
                let timer = Timer::at(until, self.id);
                *self.timer.borrow_mut() = timer;
            }
            Progress::Yielded => {
                let _ = self.after.port2().post_message(&JsValue::UNDEFINED);
            }
        }
    }
}

impl Drop for Shared {
    fn drop(&mut self) {
        let _ = ROOTS.try_with(|roots| roots.borrow_mut().remove(&self.id));
        self.after.port1().set_onmessage(None);
        self.timer.get_mut().take();
        self.core.get_mut().guest.stop();
    }
}

/// A `setTimeout` that rings a root, cleared when dropped.
struct Timer {
    handle: JsValue,
    _ring: Closure<dyn FnMut()>,
}

impl Timer {
    fn at(until: Instant, id: u64) -> Option<Self> {
        let global = js_sys::global();
        let set: Function = Reflect::get(&global, &"setTimeout".into())
            .ok()?
            .dyn_into()
            .ok()?;
        let ring = Closure::<dyn FnMut()>::new(move || ring(id));
        // Rounded up, and a millisecond over, so the root is never resumed just before the moment
        // and left without a timer; one resumed early sets another.
        let left = until.saturating_duration_since(Instant::now());
        let milliseconds = (left.as_secs_f64() * 1000.0).ceil() + 1.0;
        let handle = set
            .call2(&global, ring.as_ref(), &milliseconds.into())
            .ok()?;
        Some(Self {
            handle,
            _ring: ring,
        })
    }
}

impl Drop for Timer {
    fn drop(&mut self) {
        let global = js_sys::global();
        if let Ok(clear) = Reflect::get(&global, &"clearTimeout".into())
            && let Ok(clear) = clear.dyn_into::<Function>()
        {
            let _ = clear.call1(&global, &self.handle);
        }
    }
}

/// What the root's transport and its host share: its memory, the frames each way, and whether
/// it woke itself or panicked.
struct Transport {
    memory: RefCell<Option<WebAssembly::Memory>>,
    inbox: RefCell<VecDeque<Vec<u8>>>,
    outbox: RefCell<Vec<Vec<u8>>>,
    woken: Cell<bool>,
    panicked: RefCell<Option<String>>,
    wake: Wake,
}

impl Transport {
    /// `length` bytes of the root's memory at `pointer`.
    fn read(&self, pointer: u32, length: u32) -> Vec<u8> {
        let Some(memory) = &*self.memory.borrow() else {
            return Vec::new();
        };
        Uint8Array::new(&memory.buffer())
            .subarray(pointer, pointer.saturating_add(length))
            .to_vec()
    }
}

/// The root guest instantiated on the page: a pure guest (guides/app_libraries.md, Pure guests),
/// whose only imports are its transport, which this answers as be-wasm-guest-host's worker
/// runtime (src/guest.js) answers it in a worker.
struct PageGuest {
    exports: Object,
    transport: Rc<Transport>,
    /// The transport's imports, which live as long as the instance.
    _imports: Vec<Box<dyn std::any::Any>>,
    stopped: bool,
}

/// The value the transport's `host_receive` answers when it has no frame.
const NO_MESSAGE: i64 = -1;

impl PageGuest {
    async fn start(module: &WebAssembly::Module, wake: Wake) -> Result<Self, String> {
        let transport = Rc::new(Transport {
            memory: RefCell::new(None),
            inbox: RefCell::new(VecDeque::new()),
            outbox: RefCell::new(Vec::new()),
            woken: Cell::new(false),
            panicked: RefCell::new(None),
            wake,
        });
        let held = Rc::clone(&transport);
        let send = Closure::<dyn FnMut(u32, u32)>::new(move |pointer, length| {
            let frame = held.read(pointer, length);
            held.outbox.borrow_mut().push(frame);
        });
        let held = Rc::clone(&transport);
        let receive = Closure::<dyn FnMut(u32, u32) -> i64>::new(move |pointer: u32, capacity| {
            let mut inbox = held.inbox.borrow_mut();
            let Some(frame) = inbox.front() else {
                return NO_MESSAGE;
            };
            let length = frame.len();
            if length > capacity as usize {
                return length as i64;
            }
            if let Some(memory) = &*held.memory.borrow() {
                Uint8Array::new(&memory.buffer()).set(&Uint8Array::from(frame.as_slice()), pointer);
            }
            inbox.pop_front();
            length as i64
        });
        let held = Rc::clone(&transport);
        let wake = Closure::<dyn FnMut()>::new(move || {
            held.woken.set(true);
            (held.wake)();
        });
        let held = Rc::clone(&transport);
        let panic = Closure::<dyn FnMut(u32, u32)>::new(move |pointer, length| {
            let message = String::from_utf8_lossy(&held.read(pointer, length)).into_owned();
            *held.panicked.borrow_mut() = Some(message);
        });
        let host = Object::new();
        for (name, function) in [
            ("host_send", send.as_ref()),
            ("host_receive", receive.as_ref()),
            ("host_wake", wake.as_ref()),
            ("host_panic", panic.as_ref()),
        ] {
            let _ = Reflect::set(&host, &name.into(), function);
        }
        let imports = Object::new();
        let _ = Reflect::set(&imports, &"be3_host".into(), &host);
        let instance = JsFuture::from(WebAssembly::instantiate_module(module, &imports))
            .await
            .map_err(|error| format!("the root could not be instantiated: {}", describe(&error)))?;
        let instance: WebAssembly::Instance = instance.unchecked_into();
        let exports = instance.exports();
        let memory = Reflect::get(&exports, &"memory".into())
            .ok()
            .and_then(|memory| memory.dyn_into::<WebAssembly::Memory>().ok())
            .ok_or("the root exports no memory")?;
        *transport.memory.borrow_mut() = Some(memory);
        let mut guest = Self {
            exports,
            transport,
            _imports: vec![
                Box::new(send),
                Box::new(receive),
                Box::new(wake),
                Box::new(panic),
            ],
            stopped: false,
        };
        // A standalone module's constructors, as guest.js runs them.
        if guest.has("_initialize") {
            guest.call("_initialize")?;
        } else if guest.has("__wasm_call_ctors") {
            guest.call("__wasm_call_ctors")?;
        }
        guest.call("app_start")?;
        Ok(guest)
    }

    fn export(&self, name: &str) -> Option<Function> {
        Reflect::get(&self.exports, &name.into())
            .ok()?
            .dyn_into()
            .ok()
    }

    fn has(&self, name: &str) -> bool {
        self.export(name).is_some()
    }

    /// Runs the export `name` to its end, or says why it trapped or panicked.
    fn call(&mut self, name: &str) -> Result<(), String> {
        if self.stopped {
            return Err("the root stopped".to_owned());
        }
        let export = self
            .export(name)
            .ok_or_else(|| format!("the root exports no {name}"))?;
        export
            .call0(&JsValue::UNDEFINED)
            .map(drop)
            .map_err(|error| {
                self.stopped = true;
                self.transport
                    .panicked
                    .borrow_mut()
                    .take()
                    .unwrap_or_else(|| format!("the root trapped: {}", describe(&error)))
            })
    }

    fn stop(&mut self) {
        if !self.stopped && self.has("app_shutdown") {
            let _ = self.call("app_shutdown");
        }
        self.stopped = true;
    }
}

impl Inline for PageGuest {
    fn deliver(&mut self, frames: Vec<Vec<u8>>) {
        self.transport.inbox.borrow_mut().extend(frames);
    }

    fn step(&mut self) -> Result<(), String> {
        self.call("app_step")
    }

    fn take_outbound(&mut self) -> Vec<Vec<u8>> {
        std::mem::take(&mut *self.transport.outbox.borrow_mut())
    }

    fn take_wake(&mut self) -> bool {
        self.transport.woken.replace(false)
    }
}

fn describe(error: &JsValue) -> String {
    error
        .dyn_ref::<js_sys::Error>()
        .map(|error| String::from(error.message()))
        .or_else(|| error.as_string())
        .unwrap_or_else(|| "the browser gave no reason".to_owned())
}
