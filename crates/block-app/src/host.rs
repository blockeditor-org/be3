use beui::styled::DocumentTheme;
use std::{
    cell::RefCell,
    sync::{
        OnceLock,
        mpsc::{self, Receiver, SendError},
    },
    time::{Duration, Instant},
};

use beui::{Document, FileFilter, FilePick, FilePickId, PointerButton, Waker};
use uuid::Uuid;

static WAKER: OnceLock<Waker> = OnceLock::new();

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
    file_picks: Vec<(FileFilter, PickSlot)>,
    grab: Option<bool>,
    repaint: bool,
    repaint_after: Option<Duration>,
}

struct Host {
    started: Option<Instant>,
    now: Duration,
    pass: u64,
    pixels_per_point: f32,
    screen_scale: f32,
    dark: bool,
    grabbed: bool,
    commands: Vec<HostCommand>,
    waiting_picks: Vec<(FilePickId, PickSlot)>,
    output: Output,
}

impl Default for Host {
    fn default() -> Self {
        Self {
            started: None,
            now: Duration::ZERO,
            pass: 0,
            pixels_per_point: 1.0,
            screen_scale: 1.0,
            dark: true,
            grabbed: false,
            commands: Vec::new(),
            waiting_picks: Vec::new(),
            output: Output::default(),
        }
    }
}

fn with<R>(act: impl FnOnce(&mut Host) -> R) -> R {
    HOST.with(|host| act(&mut host.borrow_mut()))
}

struct Frame {
    now: Option<Instant>,
    dark: bool,
    pixels_per_point: f32,
    screen_scale: f32,
}

impl Default for Frame {
    fn default() -> Self {
        Self {
            now: None,
            dark: true,
            pixels_per_point: 1.0,
            screen_scale: 1.0,
        }
    }
}

pub(crate) fn begin(context: &beui::Context, document: &Document) {
    let mut picked = Vec::new();
    with(|host| {
        host.waiting_picks
            .retain_mut(|(id, slot)| match context.take_file_pick(*id) {
                Some(pick) => {
                    picked.extend(slot.take().map(|deliver| (deliver, pick)));
                    false
                }
                None => true,
            });
    });
    for (deliver, pick) in picked {
        deliver(pick);
    }
    let mut frame = Frame {
        now: Some(context.now()),
        ..Frame::default()
    };
    let [red, green, blue, _] = document.theme().background.to_array();
    frame.dark = u32::from(red) + u32::from(green) + u32::from(blue) < 384;
    frame.screen_scale = context.screen_scale();
    frame.pixels_per_point = context.pixels_per_point() * frame.screen_scale;
    start(frame);
}

fn start(frame: Frame) {
    with(|host| {
        host.dark = frame.dark;
        host.pass += 1;
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
    for (filter, slot) in output.file_picks {
        let id = context.pick_file(filter);
        with(|host| host.waiting_picks.push((id, slot)));
    }
    if let Some(grab) = output.grab
        && with(|host| std::mem::replace(&mut host.grabbed, grab)) != grab
    {
        context.set_pointer_locked(grab);
    }
    if output.repaint {
        context.request_repaint();
    }
    if let Some(delay) = output.repaint_after {
        context.request_repaint_after(delay);
    }
}

pub(crate) fn pass() -> u64 {
    with(|host| host.pass)
}

pub(crate) fn now() -> Duration {
    with(|host| host.now)
}

pub(crate) fn milliseconds() -> u64 {
    now().as_millis() as u64
}

pub(crate) fn pixels_per_point() -> f32 {
    with(|host| host.pixels_per_point)
}

pub(crate) fn dark() -> bool {
    with(|host| host.dark)
}

pub(crate) fn set_fullscreen(fullscreen: bool) {
    with(|host| host.output.fullscreen = Some(fullscreen));
}

pub(crate) fn set_grab(grab: bool) {
    with(|host| host.output.grab = Some(grab));
}

pub(crate) fn copy_text(text: String) {
    with(|host| host.output.copied = Some(text));
}

type PickSlot = Option<Box<dyn FnOnce(FilePick)>>;

pub(crate) fn pick_file(filter: FileFilter, deliver: impl FnOnce(FilePick) + 'static) {
    with(|host| {
        host.output
            .file_picks
            .push((filter, Some(Box::new(deliver))))
    });
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

pub(crate) fn push_command(command: HostCommand) {
    with(|host| host.commands.push(command));
    wake();
}

pub(crate) fn take_commands() -> Vec<HostCommand> {
    with(|host| std::mem::take(&mut host.commands))
}

pub(crate) fn pointer_button_index(button: PointerButton) -> u8 {
    match button {
        PointerButton::Primary => 0,
        PointerButton::Secondary => 1,
        PointerButton::Middle => 2,
        PointerButton::Back => 3,
        PointerButton::Forward => 4,
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum HostItem {
    Notice {
        text: String,
        spinner: bool,
    },
    Error {
        text: String,
        restart: Option<String>,
    },
    Fallback {
        name: String,
        automatic: bool,
        icon: Option<String>,
    },
    Unsupported {
        block: Uuid,
        block_type: Uuid,
    },
}
