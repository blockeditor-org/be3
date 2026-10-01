use beui::styled::DocumentTheme;
use std::{
    cell::RefCell,
    sync::{
        OnceLock,
        mpsc::{self, Receiver, SendError},
    },
    time::{Duration, Instant},
};

use beui::{Document, Event, Key, PointerButton, Pos2, Waker};
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct DragPayload {
    pub(crate) block_id: Uuid,
    pub(crate) block_type: Uuid,
}

#[derive(Clone, Debug)]
pub(crate) enum HostCommand {
    RestartPlugin(String),
}

#[derive(Default)]
pub(crate) struct Input {
    pub(crate) events: Vec<Event>,
    pub(crate) pointer: Option<Pos2>,
    pub(crate) pressed: bool,
    pub(crate) primary_released: bool,
    pub(crate) primary_down: bool,
}

#[derive(Default)]
struct Output {
    fullscreen: Option<bool>,
    copied: Option<String>,
    grab: Option<bool>,
    repaint: bool,
    repaint_after: Option<Duration>,
}

struct Host {
    start: Instant,
    pass: u64,
    pixels_per_point: f32,
    screen_scale: f32,
    dark: bool,
    input: Input,
    pointer: Option<Pos2>,
    drag: Option<DragPayload>,
    grabbed: bool,
    commands: Vec<HostCommand>,
    output: Output,
}

impl Default for Host {
    fn default() -> Self {
        Self {
            start: Instant::now(),
            pass: 0,
            pixels_per_point: 1.0,
            screen_scale: 1.0,
            dark: true,
            input: Input::default(),
            pointer: None,
            drag: None,
            grabbed: false,
            commands: Vec::new(),
            output: Output::default(),
        }
    }
}

fn with<R>(act: impl FnOnce(&mut Host) -> R) -> R {
    HOST.with(|host| act(&mut host.borrow_mut()))
}

struct Frame {
    events: Vec<Event>,
    pointer: Option<Pos2>,
    pressed: bool,
    released: bool,
    down: bool,
    touch_position: Option<Pos2>,
    dark: bool,
    pixels_per_point: f32,
    screen_scale: f32,
}

impl Default for Frame {
    fn default() -> Self {
        Self {
            events: Vec::new(),
            pointer: None,
            pressed: false,
            released: false,
            down: false,
            touch_position: None,
            dark: true,
            pixels_per_point: 1.0,
            screen_scale: 1.0,
        }
    }
}

pub(crate) fn begin(context: &beui::Context, document: &Document) {
    let mut frame = context.screen_input(|input| Frame {
        events: input.events.clone(),
        pointer: input.pointer.pos,
        pressed: input.pointer.primary_pressed || input.touch.started(),
        released: input.pointer.primary_released,
        down: input.pointer.primary_down,
        touch_position: input.touch.primary_pos(),
        ..Frame::default()
    });
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
        host.pixels_per_point = frame.pixels_per_point;
        host.screen_scale = frame.screen_scale;
        host.output = Output::default();
        if frame.pointer.is_some() {
            host.pointer = frame.pointer;
        }
        host.input = Input {
            events: frame.events,
            pointer: frame.pointer.or(frame.touch_position).or(host.pointer),
            pressed: frame.pressed,
            primary_released: frame.released,
            primary_down: frame.down,
        };
    });
}

fn finish() -> Output {
    with(|host| {
        if host.input.primary_released && !host.input.primary_down {
            host.drag = None;
        }
        std::mem::take(&mut host.output)
    })
}

pub(crate) fn end(context: &beui::Context) {
    let output = finish();
    if let Some(fullscreen) = output.fullscreen {
        context.set_fullscreen(fullscreen);
    }
    if let Some(text) = output.copied {
        context.copy_text(text);
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

pub(crate) fn now() -> f64 {
    with(|host| host.start.elapsed().as_secs_f64())
}

pub(crate) fn milliseconds() -> u64 {
    (now() * 1000.0) as u64
}

pub(crate) fn pixels_per_point() -> f32 {
    with(|host| host.pixels_per_point)
}

pub(crate) fn dark() -> bool {
    with(|host| host.dark)
}

pub(crate) fn input<R>(read: impl FnOnce(&Input) -> R) -> R {
    with(|host| read(&host.input))
}

pub(crate) fn pointer() -> Option<Pos2> {
    input(|input| input.pointer)
}

pub(crate) fn key_pressed(key: Key) -> bool {
    input(|input| {
        input.events.iter().any(|event| {
            matches!(event, Event::Key { key: pressed, pressed: true, .. } if *pressed == key)
        })
    })
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

pub(crate) fn start_drag(payload: DragPayload) {
    with(|host| host.drag = Some(payload));
}

pub(crate) fn drag() -> Option<DragPayload> {
    with(|host| host.drag)
}

pub(crate) fn pressed_at() -> Option<Pos2> {
    input(|input| input.pressed.then_some(input.pointer).flatten())
}

pub(crate) fn drag_released() -> bool {
    input(|input| input.primary_released)
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
