use std::{
    cell::RefCell,
    sync::{
        Mutex, OnceLock, PoisonError,
        mpsc::{self, Receiver, SendError},
    },
    time::{Duration, Instant},
};

use ui_types::{FileFilter, FilePick, FileSave, SavedFile};

type Waker = Box<dyn Fn() + Send + Sync>;

static WAKER: OnceLock<Waker> = OnceLock::new();
static UNSENT_SAVES: Mutex<Vec<(SavedFile, SaveSlot)>> = Mutex::new(Vec::new());

thread_local! {
    static HOST: RefCell<Host> = RefCell::new(Host::default());
}

pub fn install_waker(waker: impl Fn() + Send + Sync + 'static) {
    let _ = WAKER.set(Box::new(waker));
}

pub fn wake() {
    if let Some(waker) = WAKER.get() {
        waker();
    }
}

pub struct WakingSender<T>(mpsc::Sender<T>);

impl<T> Clone for WakingSender<T> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<T> WakingSender<T> {
    pub fn send(&self, value: T) -> Result<(), SendError<T>> {
        let sent = self.0.send(value);
        wake();
        sent
    }
}

pub fn waking_channel<T>() -> (WakingSender<T>, Receiver<T>) {
    let (sender, receiver) = mpsc::channel();
    (WakingSender(sender), receiver)
}

#[derive(Clone, Debug)]
pub enum HostCommand {
    RestartPlugin(String),
}

#[derive(Default)]
pub struct Output {
    pub fullscreen: Option<bool>,
    pub copied: Option<String>,
    pub copied_image: Option<Vec<u8>>,
    pub file_picks: Vec<(FileFilter, PickSlot)>,
    pub paste: bool,
    pub grab: Option<bool>,
    pub repaint: bool,
    pub repaint_after: Option<Duration>,
    pub animating: bool,
}

struct Host {
    started: Option<Instant>,
    now: Duration,
    pass: u64,
    seen: bool,
    clock_stopped: bool,
    pixels_per_point: f32,
    screen_scale: f32,
    opens_windows: bool,
    preferences: block_plugin_api::Preferences,
    commands: Vec<HostCommand>,
    changed: bool,
    output: Output,
}

impl Default for Host {
    fn default() -> Self {
        Self {
            started: None,
            now: Duration::ZERO,
            pass: 0,
            seen: true,
            clock_stopped: false,
            pixels_per_point: 1.0,
            screen_scale: 1.0,
            opens_windows: false,
            preferences: block_plugin_api::Preferences::default(),
            commands: Vec::new(),
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
    pub seen: bool,
    pub clock_stopped: bool,
    pub preferences: block_plugin_api::Preferences,
    pub pixels_per_point: f32,
    pub screen_scale: f32,
    pub opens_windows: bool,
}

impl Default for Frame {
    fn default() -> Self {
        Self {
            now: None,
            seen: true,
            clock_stopped: false,
            preferences: block_plugin_api::Preferences::default(),
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
        host.pixels_per_point = frame.pixels_per_point;
        host.screen_scale = frame.screen_scale;
        host.opens_windows = frame.opens_windows;
        host.output = Output::default();
    });
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

pub fn preferences() -> block_plugin_api::Preferences {
    with(|host| host.preferences)
}

pub fn set_fullscreen(fullscreen: bool) {
    with(|host| host.output.fullscreen = Some(fullscreen));
}

pub fn set_grab(grab: bool) {
    with(|host| host.output.grab = Some(grab));
}

pub fn copy_text(text: String) {
    with(|host| {
        host.output.copied_image = None;
        host.output.copied = Some(text);
    });
}

pub fn copy_image(png: Vec<u8>) {
    with(|host| {
        host.output.copied = None;
        host.output.copied_image = Some(png);
    });
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

pub fn note_animating() {
    with(|host| host.output.animating = true);
}

pub fn push_command(command: HostCommand) {
    with(|host| host.commands.push(command));
    wake();
}

pub fn take_commands() -> Vec<HostCommand> {
    with(|host| std::mem::take(&mut host.commands))
}
