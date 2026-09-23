use std::{
    cell::RefCell,
    collections::HashSet,
    sync::OnceLock,
    time::{Duration, Instant},
};

use beui::{
    CursorIcon, Document, Event, ImeArea, Key, Modifiers, PointerButton, Pos2, Rect, Vec2, Waker,
};
use block_plugin_api::{EditorInstanceId, EditorRegion};
use uuid::Uuid;

use crate::plugin_host::Blit;

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

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) struct Target {
    pub(crate) instance: EditorInstanceId,
    pub(crate) region: EditorRegion,
}

#[derive(Clone, Copy)]
struct Hit {
    target: Target,
    rect: Rect,
    clip: Rect,
    layer: u8,
}

impl Hit {
    fn contains(&self, position: Pos2) -> bool {
        self.rect.intersect(self.clip).contains(position)
    }
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
    pub(crate) delta: Vec2,
    pub(crate) modifiers: Modifiers,
    pub(crate) primary_pressed: bool,
    pub(crate) primary_released: bool,
    pub(crate) primary_down: bool,
    pub(crate) middle_down: bool,
    pub(crate) scroll: Vec2,
    pub(crate) zoom: f32,
    pub(crate) pinch: Option<Pinch>,
    pub(crate) files_hovered: bool,
    pub(crate) files_dropped: Vec<beui::DroppedFile>,
}

#[derive(Clone, Copy)]
pub(crate) struct Pinch {
    pub(crate) center: Pos2,
    pub(crate) zoom: f32,
    pub(crate) pan: Vec2,
}

#[derive(Default)]
struct Output {
    cursor: Option<CursorIcon>,
    ime: Option<ImeArea>,
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
    dark: bool,
    input: Input,
    modal: bool,
    floating: Vec<Rect>,
    focus: Option<Target>,
    previous: Vec<Hit>,
    hits: Vec<Hit>,
    keys_down: HashSet<Key>,
    files_hovering: bool,
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
            dark: true,
            input: Input::default(),
            modal: false,
            floating: Vec::new(),
            focus: None,
            previous: Vec::new(),
            hits: Vec::new(),
            keys_down: HashSet::new(),
            files_hovering: false,
            pointer: None,
            drag: None,
            grabbed: false,
            commands: Vec::new(),
            output: Output::default(),
        }
    }
}

impl Host {
    fn raised(&self, position: Pos2) -> bool {
        self.previous
            .iter()
            .any(|hit| hit.layer > 0 && hit.contains(position))
    }

    fn claimed(&self, position: Pos2) -> bool {
        (self.modal || self.floating.iter().any(|rect| rect.contains(position)))
            && !self.raised(position)
    }

    fn topmost(&self, position: Pos2) -> Option<Target> {
        let raised = self.raised(position);
        if !raised && self.claimed(position) {
            return None;
        }
        self.previous
            .iter()
            .rev()
            .filter(|hit| !raised || hit.layer > 0)
            .find(|hit| hit.contains(position))
            .map(|hit| hit.target)
    }

    fn focus_layer(&self) -> u8 {
        self.previous
            .iter()
            .find(|hit| Some(hit.target) == self.focus)
            .map_or(0, |hit| hit.layer)
    }
}

fn with<R>(act: impl FnOnce(&mut Host) -> R) -> R {
    HOST.with(|host| act(&mut host.borrow_mut()))
}

struct Frame {
    events: Vec<Event>,
    pointer: Option<Pos2>,
    modifiers: Modifiers,
    pressed: bool,
    released: bool,
    down: bool,
    middle: bool,
    scroll: Vec2,
    zoom: f32,
    pinch: Option<Pinch>,
    touch_started: bool,
    touch_position: Option<Pos2>,
    floating: Vec<Rect>,
    modal: bool,
    dark: bool,
    pixels_per_point: f32,
}

impl Default for Frame {
    fn default() -> Self {
        Self {
            events: Vec::new(),
            pointer: None,
            modifiers: Modifiers::NONE,
            pressed: false,
            released: false,
            down: false,
            middle: false,
            scroll: Vec2::ZERO,
            zoom: 1.0,
            pinch: None,
            touch_started: false,
            touch_position: None,
            floating: Vec::new(),
            modal: false,
            dark: true,
            pixels_per_point: 1.0,
        }
    }
}

pub(crate) fn begin(context: &beui::Context, document: &Document) {
    let mut frame = context.input(|input| {
        let touch = &input.touch;
        Frame {
            events: input.events.clone(),
            pointer: input.pointer.pos,
            modifiers: input.modifiers,
            pressed: input.pointer.primary_pressed,
            released: input.pointer.primary_released,
            down: input.pointer.primary_down,
            middle: input.pointer.middle_down,
            scroll: input.scroll_delta,
            zoom: input.zoom_factor,
            pinch: touch.pinch_center().map(|center| Pinch {
                center,
                zoom: touch.pinch(),
                pan: touch.pinch_pan(),
            }),
            touch_started: touch.started(),
            touch_position: touch.primary_pos(),
            ..Frame::default()
        }
    });
    frame.floating = document.floating_rects();
    frame.modal = document.modal_open();
    let [red, green, blue, _] = document.theme().background.to_array();
    frame.dark = u32::from(red) + u32::from(green) + u32::from(blue) < 384;
    frame.pixels_per_point = context.pixels_per_point();
    start(frame);
}

fn start(frame: Frame) {
    with(|host| {
        host.dark = frame.dark;
        host.pass += 1;
        host.pixels_per_point = frame.pixels_per_point;
        host.modal = frame.modal;
        host.floating = frame.floating;
        host.output = Output::default();
        let mut files_dropped = Vec::new();
        for event in &frame.events {
            match event {
                Event::Key { key, pressed, .. } => {
                    match pressed {
                        true => host.keys_down.insert(*key),
                        false => host.keys_down.remove(key),
                    };
                }
                Event::Focus(false) => host.keys_down.clear(),
                Event::FileHovered => host.files_hovering = true,
                Event::FileHoverCancelled => host.files_hovering = false,
                Event::FileDropped(file) => {
                    host.files_hovering = false;
                    files_dropped.push(file.clone());
                }
                _ => {}
            }
        }
        let delta = match (host.pointer, frame.pointer) {
            (Some(previous), Some(current)) => current - previous,
            _ => Vec2::ZERO,
        };
        if frame.pointer.is_some() {
            host.pointer = frame.pointer;
        }
        let pointer = frame.pointer.or(frame.touch_position).or(host.pointer);
        let pressed_at = match (frame.pressed, frame.touch_started) {
            (true, _) => pointer,
            (false, true) => frame.touch_position,
            (false, false) => None,
        };
        if let Some(position) = pressed_at {
            host.focus = host.topmost(position);
        }
        host.input = Input {
            events: frame.events,
            pointer,
            delta,
            modifiers: frame.modifiers,
            primary_pressed: frame.pressed,
            primary_released: frame.released,
            primary_down: frame.down,
            middle_down: frame.middle,
            scroll: frame.scroll,
            zoom: frame.zoom,
            pinch: frame.pinch,
            files_hovered: host.files_hovering,
            files_dropped,
        };
    });
}

fn finish() -> Output {
    with(|host| {
        host.previous = std::mem::take(&mut host.hits);
        if host.input.primary_released && !host.input.primary_down {
            host.drag = None;
        }
        std::mem::take(&mut host.output)
    })
}

#[cfg(test)]
pub(crate) fn test_frame(events: Vec<Event>, pointer: Option<Pos2>, pressed: bool) {
    test_frame_with(events, pointer, pressed, false);
}

#[cfg(test)]
pub(crate) fn test_frame_under_modal(events: Vec<Event>, pointer: Option<Pos2>, pressed: bool) {
    test_frame_with(events, pointer, pressed, true);
}

#[cfg(test)]
fn test_frame_with(events: Vec<Event>, pointer: Option<Pos2>, pressed: bool, modal: bool) {
    finish();
    let modifiers = events
        .iter()
        .find_map(|event| match event {
            Event::Key { modifiers, .. } => Some(*modifiers),
            _ => None,
        })
        .unwrap_or(Modifiers::NONE);
    start(Frame {
        events,
        pointer,
        modifiers,
        pressed,
        down: pressed,
        modal,
        ..Frame::default()
    });
}

pub(crate) fn keyboard_captured() -> bool {
    with(|host| host.focus.is_some() && (!host.modal || host.focus_layer() > 0))
}

pub(crate) fn filter_document_input(context: &beui::Context) {
    if !keyboard_captured() {
        return;
    }
    context.retain_events(|event| {
        !matches!(
            event,
            Event::Key { .. } | Event::Text(_) | Event::Ime(_) | Event::PointerMotion(_)
        )
    });
}

pub(crate) fn end(context: &beui::Context) {
    let output = finish();
    if let Some(cursor) = output.cursor
        && context.cursor_icon() == CursorIcon::Default
    {
        context.set_cursor_icon(cursor);
    }
    if output.ime.is_some() {
        context.set_ime_area(output.ime);
    }
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

pub(crate) fn key_down(key: Key) -> bool {
    with(|host| host.keys_down.contains(&key))
}

pub(crate) fn key_pressed(key: Key) -> bool {
    input(|input| {
        input.events.iter().any(|event| {
            matches!(event, Event::Key { key: pressed, pressed: true, .. } if *pressed == key)
        })
    })
}

pub(crate) fn consume_key(modifiers: Modifiers, key: Key) -> bool {
    with(|host| {
        let index = host.input.events.iter().position(|event| {
            matches!(
                event,
                Event::Key { key: pressed_key, pressed: true, modifiers: held, .. }
                    if *pressed_key == key && *held == modifiers
            )
        });
        match index {
            Some(index) => {
                host.input.events.remove(index);
                true
            }
            None => false,
        }
    })
}

pub(crate) fn floats(rect: Rect) -> bool {
    with(|host| {
        host.floating
            .iter()
            .any(|floating| floating.contains_rect(rect))
    })
}

pub(crate) fn claimed(position: Pos2) -> bool {
    with(|host| host.claimed(position))
}

pub(crate) fn register(target: Target, rect: Rect, clip: Rect, layer: u8) {
    with(|host| {
        host.hits.push(Hit {
            target,
            rect,
            clip,
            layer,
        });
    });
}

pub(crate) fn reaches(target: Target, position: Pos2) -> bool {
    with(|host| host.topmost(position) == Some(target))
}

pub(crate) fn hovered(target: Target) -> bool {
    with(|host| {
        host.input
            .pointer
            .is_some_and(|position| host.topmost(position) == Some(target))
    })
}

pub(crate) fn focused(target: Target) -> bool {
    with(|host| host.focus == Some(target))
}

pub(crate) fn focus() -> Option<Target> {
    with(|host| host.focus)
}

pub(crate) fn request_focus(target: Target) {
    with(|host| host.focus = Some(target));
}

pub(crate) fn clear_focus() {
    with(|host| host.focus = None);
}

pub(crate) fn set_cursor(cursor: CursorIcon) {
    with(|host| host.output.cursor = Some(cursor));
}

pub(crate) fn set_ime(area: ImeArea) {
    with(|host| host.output.ime = Some(area));
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

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct PlacedItem {
    pub(crate) rect: Rect,
    pub(crate) item: HostItem,
}

#[derive(Default)]
pub(crate) struct SurfaceOutput {
    pub(crate) blits: Vec<Blit>,
    pub(crate) items: Vec<(u64, PlacedItem)>,
}

pub(crate) struct Ui<'a> {
    output: &'a mut SurfaceOutput,
    rect: Rect,
    clip: Rect,
    layer: u8,
}

impl<'a> Ui<'a> {
    pub(crate) fn new(output: &'a mut SurfaceOutput, rect: Rect, clip: Rect, layer: u8) -> Self {
        Self {
            output,
            rect,
            clip,
            layer,
        }
    }

    pub(crate) fn register(&self, target: Target, rect: Rect) {
        register(target, rect, self.clip, self.layer);
    }

    pub(crate) fn rect(&self) -> Rect {
        self.rect
    }

    pub(crate) fn clip(&self) -> Rect {
        self.clip
    }

    pub(crate) fn child(&mut self, rect: Rect, clip: Rect) -> Ui<'_> {
        Ui {
            output: self.output,
            rect,
            clip: clip.intersect(self.clip),
            layer: self.layer,
        }
    }

    pub(crate) fn blit(&mut self, blit: Blit) {
        self.output.blits.push(blit);
    }

    pub(crate) fn item(&mut self, key: impl std::hash::Hash, rect: Rect, item: HostItem) {
        let rect = rect.intersect(self.clip);
        if !rect.is_positive() {
            return;
        }
        let key = {
            use std::hash::{DefaultHasher, Hasher};
            let mut hasher = DefaultHasher::new();
            key.hash(&mut hasher);
            hasher.finish()
        };
        if self
            .output
            .items
            .iter()
            .any(|(existing, _)| *existing == key)
        {
            return;
        }
        self.output.items.push((key, PlacedItem { rect, item }));
    }
}
