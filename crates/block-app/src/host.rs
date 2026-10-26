use std::{
    cell::RefCell,
    sync::{
        Mutex, OnceLock, PoisonError,
        mpsc::{self, Receiver, SendError},
    },
    time::{Duration, Instant},
};

use beui::{FileFilter, FilePick, FilePickId, FileSave, FileSaveId, SavedFile, Waker};
use uuid::Uuid;

static WAKER: OnceLock<Waker> = OnceLock::new();
static UNSENT_SAVES: Mutex<Vec<(SavedFile, SaveSlot)>> = Mutex::new(Vec::new());

thread_local! {
    static HOST: RefCell<Host> = RefCell::new(Host::default());
}

pub(crate) fn install_waker(waker: Waker) {
    let _ = WAKER.set(waker);
}

pub(crate) fn wake() {
    if let Some(waker) = WAKER.get() {
        waker.wake();
    }
}

pub(crate) struct WakingSender<T>(mpsc::Sender<T>);

impl<T> Clone for WakingSender<T> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<T> WakingSender<T> {
    pub(crate) fn send(&self, value: T) -> Result<(), SendError<T>> {
        let sent = self.0.send(value);
        wake();
        sent
    }
}

pub(crate) fn waking_channel<T>() -> (WakingSender<T>, Receiver<T>) {
    let (sender, receiver) = mpsc::channel();
    (WakingSender(sender), receiver)
}

#[derive(Clone, Debug)]
pub(crate) enum HostCommand {
    RestartPlugin(String),
}

#[derive(Default)]
struct Output {
    fullscreen: Option<bool>,
    copied: Option<String>,
    copied_image: Option<Vec<u8>>,
    file_picks: Vec<(FileFilter, PickSlot)>,
    paste: bool,
    grab: Option<bool>,
    repaint: bool,
    repaint_after: Option<Duration>,
    animating: bool,
}

struct Host {
    started: Option<Instant>,
    now: Duration,
    pass: u64,
    seen: bool,
    clock_stopped: bool,
    pixels_per_point: f32,
    screen_scale: f32,
    preferences: block_plugin_api::Preferences,
    commands: Vec<HostCommand>,
    waiting_picks: Vec<(FilePickId, PickSlot)>,
    waiting_saves: Vec<(FileSaveId, SaveSlot)>,
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
            preferences: block_plugin_api::Preferences::default(),
            commands: Vec::new(),
            waiting_picks: Vec::new(),
            waiting_saves: Vec::new(),
            output: Output::default(),
        }
    }
}

fn with<R>(act: impl FnOnce(&mut Host) -> R) -> R {
    HOST.with(|host| act(&mut host.borrow_mut()))
}

struct Frame {
    now: Option<Instant>,
    seen: bool,
    clock_stopped: bool,
    preferences: block_plugin_api::Preferences,
    pixels_per_point: f32,
    screen_scale: f32,
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
        }
    }
}

pub(crate) fn begin(context: &beui::Context) {
    let mut picked = Vec::new();
    let mut saved = Vec::new();
    with(|host| {
        host.waiting_picks
            .retain_mut(|(id, slot)| match context.take_file_pick(*id) {
                Some(pick) => {
                    picked.extend(slot.take().map(|deliver| (deliver, pick)));
                    false
                }
                None => true,
            });
        for (id, deliver) in std::mem::take(&mut host.waiting_saves) {
            match context.take_file_save(id) {
                Some(save) => saved.push((deliver, save)),
                None => host.waiting_saves.push((id, deliver)),
            }
        }
    });
    for (deliver, pick) in picked {
        deliver(pick);
    }
    for (deliver, save) in saved {
        deliver(save);
    }
    let mut frame = Frame {
        now: Some(context.now()),
        seen: context.seen(),
        clock_stopped: context.clock_stopped(),
        ..Frame::default()
    };
    frame.preferences = context.preferences();
    let layers = context.preference_layers();
    crate::plugin_host::publish::<block_plugin_api::PreferenceSourcesWatch>(
        &block_plugin_api::HostPreferenceSources {
            system: layers.layer(beui::PreferenceSource::System),
            developer: layers.layer(beui::PreferenceSource::Developer),
        },
    );
    frame.screen_scale = context.screen_scale();
    frame.pixels_per_point = context.pixels_per_point() * frame.screen_scale;
    start(frame);
}

fn start(frame: Frame) {
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
        host.output = Output::default();
    });
}

fn finish() -> Output {
    with(|host| std::mem::take(&mut host.output))
}

pub(crate) fn end(context: &beui::Context) {
    let output = finish();
    if let Some(fullscreen) = output.fullscreen {
        context.set_fullscreen(fullscreen);
    }
    if let Some(text) = output.copied {
        context.copy_text(text);
    }
    if let Some(png) = output.copied_image {
        context.copy_image(png);
    }
    for (filter, slot) in output.file_picks {
        let id = context.pick_file(filter);
        with(|host| host.waiting_picks.push((id, slot)));
    }
    let saves = std::mem::take(&mut *UNSENT_SAVES.lock().unwrap_or_else(PoisonError::into_inner));
    for (file, deliver) in saves {
        let id = context.save_file(file);
        with(|host| host.waiting_saves.push((id, deliver)));
    }
    if output.paste {
        context.request_paste();
    }
    if let Some(grab) = output.grab {
        context.set_pointer_locked(grab);
    }
    if output.repaint {
        context.request_repaint();
    }
    if let Some(delay) = output.repaint_after {
        context.request_repaint_after(delay);
    }
    if output.animating {
        context.note_animating();
    }
}

pub(crate) fn pass() -> u64 {
    with(|host| host.pass)
}

pub(crate) fn seen() -> bool {
    with(|host| host.seen)
}

pub(crate) fn clock_stopped() -> bool {
    with(|host| host.clock_stopped)
}

pub(crate) fn now() -> Duration {
    with(|host| host.now)
}

pub(crate) fn micros_at(at: Instant) -> u64 {
    with(|host| match host.started {
        Some(started) => at.saturating_duration_since(started),
        None => host.now,
    })
    .as_micros() as u64
}

pub(crate) fn milliseconds() -> u64 {
    now().as_millis() as u64
}

pub(crate) fn pixels_per_point() -> f32 {
    with(|host| host.pixels_per_point)
}

pub(crate) fn preferences() -> block_plugin_api::Preferences {
    with(|host| host.preferences)
}

pub(crate) fn set_fullscreen(fullscreen: bool) {
    with(|host| host.output.fullscreen = Some(fullscreen));
}

pub(crate) fn set_grab(grab: bool) {
    with(|host| host.output.grab = Some(grab));
}

pub(crate) fn copy_text(text: String) {
    with(|host| {
        host.output.copied_image = None;
        host.output.copied = Some(text);
    });
}

pub(crate) fn copy_image(png: Vec<u8>) {
    with(|host| {
        host.output.copied = None;
        host.output.copied_image = Some(png);
    });
}

type PickSlot = Option<Box<dyn FnOnce(FilePick)>>;
type SaveSlot = Box<dyn FnOnce(FileSave) + Send>;

pub(crate) fn request_paste() {
    with(|host| host.output.paste = true);
}

pub(crate) fn pick_file(filter: FileFilter, deliver: impl FnOnce(FilePick) + 'static) {
    with(|host| {
        host.output
            .file_picks
            .push((filter, Some(Box::new(deliver))))
    });
}

pub(crate) fn save_file(file: SavedFile, deliver: impl FnOnce(FileSave) + Send + 'static) {
    UNSENT_SAVES
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .push((file, Box::new(deliver)));
    wake();
}

pub(crate) fn request_repaint() {
    with(|host| host.output.repaint = true);
}

pub(crate) fn request_repaint_after(delay: Duration) {
    with(|host| {
        host.output.repaint_after = Some(match host.output.repaint_after {
            Some(current) => current.min(delay),
            None => delay,
        });
    });
}

pub(crate) fn note_animating() {
    with(|host| host.output.animating = true);
}

pub(crate) fn push_command(command: HostCommand) {
    with(|host| host.commands.push(command));
    wake();
}

pub(crate) fn take_commands() -> Vec<HostCommand> {
    with(|host| std::mem::take(&mut host.commands))
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum HostItem {
    Notice {
        text: String,
        spinner: bool,
        icon: Option<beui::icons::Glyph>,
    },
    Unsupported {
        block: Uuid,
        block_type: Uuid,
    },
}
