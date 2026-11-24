use std::{
    any::Any,
    cell::{Cell, RefCell},
    collections::HashMap,
    marker::PhantomData,
    rc::Rc,
    sync::{Mutex, OnceLock, PoisonError, RwLock},
    thread::{self, ThreadId},
    time::{Duration, Instant},
};

use be_lib_ui_types::{FileFilter, FilePick, FileSave, SavedFile};

type Waker = Box<dyn Fn() + Send + Sync>;

/// Every waker installed (`install_waker`): one per host in a process, and in a test process
/// each test's own, so none silently goes unwoken. Written only as one is installed.
static WAKERS: RwLock<Vec<Waker>> = RwLock::new(Vec::new());
static UNSENT_SAVES: Mutex<Vec<(SavedFile, SaveSlot)>> = Mutex::new(Vec::new());

type Letter = (u64, Box<dyn Any + Send>);
type Receive = Rc<dyn Fn(&mut dyn Any, Box<dyn Any + Send>)>;

// How a letter reaches the thread that made its mailbox: one API, two transports. Natively
// every thread shares one locked list. On the web the page takes no lock another thread takes,
// so a worker's letter comes to the page by `postMessage` and waits in the page's own list.
#[cfg(not(target_arch = "wasm32"))]
mod letters_native;
#[cfg(not(target_arch = "wasm32"))]
use letters_native as letters;
#[cfg(not(target_arch = "wasm32"))]
pub use letters_native::{letters_posted, wait_for_letter};
#[cfg(target_arch = "wasm32")]
mod letters_web;
#[cfg(target_arch = "wasm32")]
use letters_web as letters;

/// A letter in bytes (`Across`), which the mailbox it is for decodes as it delivers it.
struct Encoded(Vec<u8>);

type Decode = fn(&[u8]) -> Box<dyn Any + Send>;

thread_local! {
    static HOST: RefCell<Host> = RefCell::new(Host::default());
    static MAILBOXES: RefCell<HashMap<u64, (u64, Receive)>> = RefCell::new(HashMap::new());
    /// How each mailbox a thread's letters arrive at in bytes decodes them (`Post::across`).
    static DECODERS: RefCell<HashMap<u64, Decode>> = RefCell::new(HashMap::new());
    static NEXT_MAILBOX: Cell<u64> = const { Cell::new(1) };
}

/// Adds `waker` to those `wake` calls: each host (or test) installs its own, and a later one
/// does not take an earlier one's place.
pub fn install_waker(waker: impl Fn() + Send + Sync + 'static) {
    WAKERS
        .write()
        .unwrap_or_else(PoisonError::into_inner)
        .push(Box::new(waker));
}

static CLOCK: OnceLock<fn() -> Instant> = OnceLock::new();

pub fn install_clock(clock: fn() -> Instant) {
    let _ = CLOCK.set(clock);
}

pub fn clock() -> Instant {
    CLOCK.get().map_or_else(Instant::now, |clock| clock())
}

pub fn wake() {
    for waker in WAKERS.read().unwrap_or_else(PoisonError::into_inner).iter() {
        waker();
    }
}

pub struct Mailbox(u64);

impl Drop for Mailbox {
    fn drop(&mut self) {
        let _ = MAILBOXES.try_with(|mailboxes| mailboxes.borrow_mut().remove(&self.0));
        let _ = DECODERS.try_with(|decoders| decoders.borrow_mut().remove(&self.0));
    }
}

pub struct Post<T> {
    mailbox: u64,
    thread: ThreadId,
    kind: PhantomData<fn(T)>,
}

impl<T> Clone for Post<T> {
    fn clone(&self) -> Self {
        Self {
            mailbox: self.mailbox,
            thread: self.thread,
            kind: PhantomData,
        }
    }
}

impl<T: Send + 'static> Post<T> {
    /// Whether its mailbox still takes letters: false once the mailbox is dropped. Only the
    /// thread that made the mailbox can tell; elsewhere it answers true.
    pub fn open(&self) -> bool {
        self.thread != thread::current().id()
            || MAILBOXES
                .try_with(|mailboxes| mailboxes.borrow().contains_key(&self.mailbox))
                .unwrap_or(false)
    }

    pub fn send(&self, value: T) {
        letters::post(self.thread, (self.mailbox, Box::new(value)));
    }
}

impl<T: Send + serde::Serialize + serde::de::DeserializeOwned + 'static> Post<T> {
    /// The post for another thread to send to this mailbox: its letters travel as bytes, as
    /// they must to a page whose memory the thread does not share
    /// (decisions/the_web_page_never_waits.md). Made on the mailbox's own thread.
    pub fn across(&self) -> Across<T> {
        assert_eq!(
            self.thread,
            thread::current().id(),
            "a mailbox's post across threads is made on the mailbox's thread"
        );
        DECODERS.with_borrow_mut(|decoders| {
            decoders.insert(self.mailbox, |bytes| {
                // Encoded from a `T` by `Across::send`: bytes that do not decode are a bug.
                Box::new(postcard::from_bytes::<T>(bytes).expect("a letter decodes"))
            })
        });
        Across {
            mailbox: self.mailbox,
            #[cfg(not(target_arch = "wasm32"))]
            thread: self.thread,
            kind: PhantomData,
        }
    }
}

/// A post another thread sends to a mailbox with (`Post::across`): each letter goes as its
/// bytes, and the mailbox decodes it as it delivers it. On the web this is the only way a
/// worker's letter reaches the page, and since only the page makes mailboxes there, an `Across`
/// is its mailbox's number alone: it travels to a worker as bytes too (its serde form), in
/// what the worker starts from (`be_host_stack::start_in_worker`).
pub struct Across<T> {
    mailbox: u64,
    /// Natively, the thread whose letters the mailbox is among.
    #[cfg(not(target_arch = "wasm32"))]
    thread: ThreadId,
    kind: PhantomData<fn(T)>,
}

impl<T> Clone for Across<T> {
    fn clone(&self) -> Self {
        Self {
            mailbox: self.mailbox,
            #[cfg(not(target_arch = "wasm32"))]
            thread: self.thread,
            kind: PhantomData,
        }
    }
}

impl<T: serde::Serialize> Across<T> {
    pub fn send(&self, value: T) {
        let bytes = postcard::to_stdvec(&value).expect("a letter encodes");
        #[cfg(not(target_arch = "wasm32"))]
        letters::post_bytes(self.thread, self.mailbox, bytes);
        #[cfg(target_arch = "wasm32")]
        letters::post_bytes(self.mailbox, bytes);
    }
}

#[cfg(target_arch = "wasm32")]
impl<T> serde::Serialize for Across<T> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_u64(self.mailbox)
    }
}

#[cfg(target_arch = "wasm32")]
impl<'de, T> serde::Deserialize<'de> for Across<T> {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Ok(Self {
            mailbox: <u64 as serde::Deserialize>::deserialize(deserializer)?,
            kind: PhantomData,
        })
    }
}

/// A letter as its mailbox takes it: one that came in bytes decoded (`Across`).
fn decoded((mailbox, letter): Letter) -> Letter {
    match letter.downcast::<Encoded>() {
        Ok(encoded) => match DECODERS.with_borrow(|decoders| decoders.get(&mailbox).copied()) {
            Some(decode) => (mailbox, decode(&encoded.0)),
            None => (mailbox, encoded),
        },
        Err(letter) => (mailbox, letter),
    }
}

fn next_mailbox() -> u64 {
    let next = NEXT_MAILBOX.get();
    NEXT_MAILBOX.set(next + 1);
    next
}

pub struct Desk<C> {
    id: u64,
    context: PhantomData<fn(&mut C)>,
}

impl<C> Clone for Desk<C> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<C> Copy for Desk<C> {}

impl<C: 'static> Default for Desk<C> {
    fn default() -> Self {
        Self {
            id: next_mailbox(),
            context: PhantomData,
        }
    }
}

impl<C: 'static> Desk<C> {
    pub fn mailbox<T: Send + 'static>(
        self,
        receive: impl Fn(&mut C, T) + 'static,
    ) -> (Mailbox, Post<T>) {
        // A worker's letter reaches only the page on the web (`letters_web.rs`), so only the
        // page may make a mailbox there.
        #[cfg(target_arch = "wasm32")]
        assert!(
            !be_wasm_threads::may_wait(),
            "a worker made a mailbox: on the web only the page makes mailboxes"
        );
        let mailbox = next_mailbox();
        let receive: Receive = Rc::new(move |context, letter| {
            if let (Some(context), Ok(value)) =
                (context.downcast_mut::<C>(), letter.downcast::<T>())
            {
                receive(context, *value);
            }
        });
        MAILBOXES.with_borrow_mut(|mailboxes| mailboxes.insert(mailbox, (self.id, receive)));
        let post = Post {
            mailbox,
            thread: thread::current().id(),
            kind: PhantomData,
        };
        (Mailbox(mailbox), post)
    }

    pub fn deliver(self, context: &mut C) {
        let here = thread::current().id();
        let at_this_desk = |mailbox: u64| {
            MAILBOXES
                .with_borrow(|mailboxes| mailboxes.get(&mailbox).map(|(desk, _)| *desk == self.id))
        };
        let letters = letters::take(here, at_this_desk);
        for letter in letters {
            let (mailbox, letter) = decoded(letter);
            let receive = MAILBOXES.with_borrow(|mailboxes| {
                mailboxes
                    .get(&mailbox)
                    .map(|(_, receive)| Rc::clone(receive))
            });
            if let Some(receive) = receive {
                receive(context, letter);
            }
        }
    }
}

const FRAME: Desk<()> = Desk {
    id: 0,
    context: PhantomData,
};

pub fn mailbox<T: Send + 'static>(receive: impl Fn(T) + 'static) -> (Mailbox, Post<T>) {
    FRAME.mailbox(move |(), value| receive(value))
}

pub fn deliver() {
    FRAME.deliver(&mut ());
}

#[derive(Default)]
pub struct Output {
    pub fullscreen: Option<bool>,
    pub copied: Option<CopiedClip>,
    pub file_picks: Vec<(FileFilter, PickSlot)>,
    pub paste: bool,
    pub grab: Option<bool>,
    pub repaint: bool,
    pub repaint_after: Option<Duration>,
    pub animating: bool,
    pub close: bool,
    pub raise: bool,
}

struct Host {
    started: Option<Instant>,
    now: Duration,
    wall: Duration,
    pass: u64,
    seen: bool,
    clock_stopped: bool,
    pixels_per_point: f32,
    screen_scale: f32,
    opens_windows: bool,
    preferences: be_sdk_api::Preferences,
    changed: bool,
    output: Output,
}

impl Default for Host {
    fn default() -> Self {
        Self {
            started: None,
            now: Duration::ZERO,
            wall: Duration::ZERO,
            pass: 0,
            seen: true,
            clock_stopped: false,
            pixels_per_point: 1.0,
            screen_scale: 1.0,
            opens_windows: false,
            preferences: be_sdk_api::Preferences::default(),
            changed: false,
            output: Output::default(),
        }
    }
}

fn with<R>(act: impl FnOnce(&mut Host) -> R) -> R {
    HOST.with(|host| act(&mut host.borrow_mut()))
}

pub struct Frame {
    pub now: Option<Instant>,
    pub wall: Option<Duration>,
    pub seen: bool,
    pub clock_stopped: bool,
    pub preferences: be_sdk_api::Preferences,
    pub pixels_per_point: f32,
    pub screen_scale: f32,
    pub opens_windows: bool,
}

impl Default for Frame {
    fn default() -> Self {
        Self {
            now: None,
            wall: None,
            seen: true,
            clock_stopped: false,
            preferences: be_sdk_api::Preferences::default(),
            pixels_per_point: 1.0,
            screen_scale: 1.0,
            opens_windows: false,
        }
    }
}

pub fn start(frame: Frame) {
    with(|host| {
        host.preferences = frame.preferences;
        host.pass += 1;
        host.seen = frame.seen;
        host.clock_stopped = frame.clock_stopped;
        if let Some(now) = frame.now {
            let started = *host.started.get_or_insert(now);
            host.now = now.saturating_duration_since(started);
        }
        if let Some(wall) = frame.wall {
            host.wall = wall;
        }
        host.pixels_per_point = frame.pixels_per_point;
        host.screen_scale = frame.screen_scale;
        host.opens_windows = frame.opens_windows;
        host.output = Output::default();
    });
    deliver();
}

pub fn finish() -> Output {
    with(|host| std::mem::take(&mut host.output))
}

pub fn take_unsent_saves() -> Vec<(SavedFile, SaveSlot)> {
    std::mem::take(&mut *UNSENT_SAVES.lock().unwrap_or_else(PoisonError::into_inner))
}

pub fn changed() {
    with(|host| host.changed = true);
    request_repaint();
}

pub fn take_changed() -> bool {
    with(|host| std::mem::take(&mut host.changed))
}

pub fn pass() -> u64 {
    with(|host| host.pass)
}

pub fn seen() -> bool {
    with(|host| host.seen)
}

pub fn clock_stopped() -> bool {
    with(|host| host.clock_stopped)
}

pub fn now() -> Duration {
    with(|host| host.now)
}

pub fn wall_clock() -> Duration {
    with(|host| host.wall)
}

pub fn micros_at(at: Instant) -> u64 {
    with(|host| match host.started {
        Some(started) => at.saturating_duration_since(started),
        None => host.now,
    })
    .as_micros() as u64
}

pub fn milliseconds() -> u64 {
    now().as_millis() as u64
}

pub fn pixels_per_point() -> f32 {
    with(|host| host.pixels_per_point)
}

pub fn opens_windows() -> bool {
    with(|host| host.opens_windows)
}

pub fn preferences() -> be_sdk_api::Preferences {
    with(|host| host.preferences)
}

pub fn set_fullscreen(fullscreen: bool) {
    with(|host| host.output.fullscreen = Some(fullscreen));
}

pub fn set_grab(grab: bool) {
    with(|host| host.output.grab = Some(grab));
}

/// What an app instance copied (`shell-message.copy`): items without data are
/// rendered by the instance on request (`render-clip` with `serial`).
#[derive(Clone, Debug)]
pub struct CopiedClip {
    pub app_id: String,
    pub instance: be_sdk_api::EditorInstanceId,
    pub serial: u64,
    pub items: Vec<be_sdk_api::OfferedClipItem>,
}

pub fn copy(clip: CopiedClip) {
    with(|host| host.output.copied = Some(clip));
}

pub type PickSlot = Option<Box<dyn FnOnce(FilePick)>>;
pub type SaveSlot = Box<dyn FnOnce(FileSave) + Send>;

pub fn request_paste() {
    with(|host| host.output.paste = true);
}

pub fn pick_file(filter: FileFilter, deliver: impl FnOnce(FilePick) + 'static) {
    with(|host| {
        host.output
            .file_picks
            .push((filter, Some(Box::new(deliver))))
    });
}

pub fn save_file(file: SavedFile, deliver: impl FnOnce(FileSave) + Send + 'static) {
    UNSENT_SAVES
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .push((file, Box::new(deliver)));
    wake();
}

pub fn request_repaint() {
    with(|host| host.output.repaint = true);
}

pub fn request_repaint_after(delay: Duration) {
    with(|host| {
        host.output.repaint_after = Some(match host.output.repaint_after {
            Some(current) => current.min(delay),
            None => delay,
        });
    });
}

pub fn close_window() {
    with(|host| host.output.close = true);
}

pub fn raise_window() {
    with(|host| host.output.raise = true);
}

pub fn note_animating() {
    with(|host| host.output.animating = true);
}

#[cfg(test)]
mod tests;
