use std::collections::VecDeque;
use std::time::{Duration, Instant};

use crate::geometry::{Pos2, Vec2};

const MULTI_CLICK_DELAY: f32 = 0.3;
const MULTI_CLICK_DISTANCE: f32 = 6.0;
const MULTI_CLICK_LIMIT: u32 = 4;
pub const TOUCH_DRAG_THRESHOLD: f32 = 8.0;
pub const LONG_PRESS_DELAY: Duration = Duration::from_millis(500);
pub const FINGER_TAP_TIME: Duration = Duration::from_millis(350);
pub const TOUCH_VELOCITY_WINDOW: f32 = 0.12;
pub const MAX_TOUCH_VELOCITY: f32 = 4_000.0;
const WHEEL_VELOCITY_WINDOW: Duration = Duration::from_millis(100);

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Key {
    ArrowDown,
    ArrowLeft,
    ArrowRight,
    ArrowUp,
    Backspace,
    BracketLeft,
    BracketRight,
    Delete,
    End,
    Enter,
    Escape,
    Home,
    Minus,
    PageDown,
    PageUp,
    Plus,
    Space,
    Tab,
    Zero,
    One,
    Two,
    Three,
    Four,
    Five,
    Six,
    Seven,
    Eight,
    Nine,
    Backtick,
    A,
    B,
    C,
    D,
    E,
    F,
    G,
    H,
    I,
    J,
    K,
    L,
    M,
    N,
    O,
    P,
    Q,
    R,
    S,
    T,
    U,
    V,
    W,
    X,
    Y,
    Z,
    Insert,
    Comma,
    Period,
    Slash,
    Backslash,
    Semicolon,
    Quote,
    BrowserBack,
    F1,
    F2,
    F3,
    F4,
    F5,
    F6,
    F7,
    F8,
    F9,
    F10,
    F11,
    F12,
    F13,
    F14,
    F15,
    F16,
    F17,
    F18,
    F19,
    F20,
    F21,
    F22,
    F23,
    F24,
    VolumeUp,
    VolumeDown,
    VolumeMute,
    MicMute,
    BrightnessUp,
    BrightnessDown,
    MediaPlayPause,
    MediaNext,
    MediaPrevious,
    MediaStop,
    Shift,
    Ctrl,
    Alt,
    Logo,
    Unidentified,
}

impl Key {
    pub const fn is_modifier(self) -> bool {
        matches!(self, Self::Shift | Self::Ctrl | Self::Alt | Self::Logo)
    }

    pub const fn is_media(self) -> bool {
        matches!(
            self,
            Self::VolumeUp
                | Self::VolumeDown
                | Self::VolumeMute
                | Self::MicMute
                | Self::BrightnessUp
                | Self::BrightnessDown
                | Self::MediaPlayPause
                | Self::MediaNext
                | Self::MediaPrevious
                | Self::MediaStop
        )
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Default, Debug)]
pub struct Modifiers {
    pub alt: bool,
    pub ctrl: bool,
    pub shift: bool,
    pub logo: bool,
}

impl Modifiers {
    pub const NONE: Self = Self {
        alt: false,
        ctrl: false,
        shift: false,
        logo: false,
    };

    pub const LOGO: Self = Self {
        logo: true,
        ..Self::NONE
    };

    pub const fn command(self) -> bool {
        self.ctrl || self.alt || self.logo
    }

    pub const ALT: Self = Self {
        alt: true,
        ..Self::NONE
    };

    pub const CTRL: Self = Self {
        ctrl: true,
        ..Self::NONE
    };

    pub const SHIFT: Self = Self {
        shift: true,
        ..Self::NONE
    };

    pub const fn holds(self, held: Self) -> bool {
        (self.alt || !held.alt)
            && (self.ctrl || !held.ctrl)
            && (self.shift || !held.shift)
            && (self.logo || !held.logo)
    }

    pub const fn any(self) -> bool {
        self.alt || self.ctrl || self.shift || self.logo
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct KeyPress {
    pub key: Key,
    pub pressed: bool,
    pub repeat: bool,
    pub modifiers: Modifiers,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct KeyChord {
    pub key: Key,
    pub modifiers: Modifiers,
    pub tap: bool,
}

impl KeyPress {
    pub const fn down(key: Key, modifiers: Modifiers) -> Self {
        Self {
            key,
            pressed: true,
            repeat: false,
            modifiers,
        }
    }

    pub const fn up(key: Key, modifiers: Modifiers) -> Self {
        Self {
            key,
            pressed: false,
            repeat: false,
            modifiers,
        }
    }
}

impl KeyChord {
    pub const fn new(key: Key, modifiers: Modifiers) -> Self {
        Self {
            key,
            modifiers,
            tap: false,
        }
    }

    pub const fn tap(key: Key) -> Self {
        Self {
            key,
            modifiers: Modifiers::NONE,
            tap: true,
        }
    }

    pub fn matches(self, press: KeyPress) -> bool {
        !self.tap && press.key == self.key && press.modifiers == self.modifiers
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct PointerPress {
    pub pos: Pos2,
    pub fraction: Vec2,
    pub clicks: u32,
    pub modifiers: Modifiers,
    pub touch: bool,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct PointerSample {
    pub pos: Pos2,
    pub touch: bool,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct ScrollGesture {
    pub delta: Vec2,
    pub fling: Vec2,
    pub pos: Pos2,
    pub modifiers: Modifiers,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct DragGesture {
    pub started: bool,
    pub ended: bool,
    pub cancelled: bool,
    pub delta: Vec2,
    pub velocity: Vec2,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct AutoscrollGesture {
    pub origin: Pos2,
    pub pos: Pos2,
    pub ended: bool,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct SecondaryDrag {
    pub from: Pos2,
    pub pos: Pos2,
    pub started: bool,
    pub ended: bool,
    pub cancelled: bool,
    pub modifiers: Modifiers,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct ZoomGesture {
    pub factor: f32,
    pub pos: Pos2,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PointerButton {
    Primary,
    Secondary,
    Middle,
    Back,
    Forward,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct TouchId {
    pub device: u64,
    pub finger: u64,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TouchPhase {
    Start,
    Move,
    End,
    Cancel,
}

#[derive(Clone, PartialEq, Debug)]
pub enum Event {
    Focus(bool),
    Modifiers(Modifiers),
    Key {
        press: KeyPress,
        code: Option<u32>,
        text: Option<String>,
    },
    PointerButton {
        pos: Pos2,
        button: PointerButton,
        pressed: bool,
        modifiers: Modifiers,
    },
    PointerGone,
    PointerMotion(Vec2),
    PointerMoved(Pos2),
    Scroll(Vec2),
    ScrollEnded,
    Text(String),
    Touch {
        id: TouchId,
        phase: TouchPhase,
        pos: Pos2,
        force: Option<f32>,
    },
    Zoom(f32),
    Ime(ImeEvent),
    FileHovered,
    FileHoverCancelled,
    FileDropped(DroppedFile),
    Back(BackGesture),
    InterceptedKey(KeyPress),
    InterceptedTap(Key),
}

impl Event {
    pub fn key(press: KeyPress) -> Self {
        Self::Key {
            press,
            code: None,
            text: None,
        }
    }

    pub fn press(&self) -> Option<KeyPress> {
        match self {
            Self::Key { press, .. } => Some(*press),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum BackGesture {
    Started { edge: BackEdge },
    Progressed(f32),
    Cancelled,
    Invoked,
}

#[derive(Clone, Copy, PartialEq, Eq, Default, Debug)]
pub enum BackEdge {
    #[default]
    None,
    Left,
    Right,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum ImeEvent {
    Enabled,
    Disabled,
    SetComposingText(String),
    CommitText(String),
    FinishComposing,
    SetComposingRegion(std::ops::Range<usize>),
    ReplaceText {
        range: std::ops::Range<usize>,
        text: String,
    },
    DeleteSurrounding {
        before: usize,
        after: usize,
    },
    SetSelection {
        anchor: usize,
        focus: usize,
    },
}

#[derive(Clone, PartialEq, Eq, Default, Debug)]
pub struct ImeText {
    pub start: usize,
    pub text: String,
    pub selection: std::ops::Range<usize>,
    pub composing: Option<std::ops::Range<usize>>,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct DroppedFile {
    pub name: String,
    pub path: Option<std::path::PathBuf>,
    pub bytes: Option<std::sync::Arc<[u8]>>,
}

#[derive(Clone, PartialEq, Debug)]
pub struct ImeArea {
    pub rect: crate::geometry::Rect,
    pub cursor: crate::geometry::Rect,
    pub text: Option<ImeText>,
    pub keyboard: bool,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Default, Debug)]
pub enum CursorIcon {
    #[default]
    Default,
    Crosshair,
    Grab,
    Grabbing,
    NotAllowed,
    PointingHand,
    ResizeHorizontal,
    ResizeVertical,
    ResizeNeSw,
    ResizeNwSe,
    Text,
    Wait,
    None,
    Move,
    Progress,
    Help,
    Alias,
    ContextMenu,
    Cell,
    VerticalText,
    Copy,
    NoDrop,
    AllScroll,
    ZoomIn,
    ZoomOut,
    ResizeColumn,
    ResizeRow,
    ResizeNorth,
    ResizeEast,
    ResizeSouth,
    ResizeWest,
    ResizeNorthEast,
    ResizeNorthWest,
    ResizeSouthEast,
    ResizeSouthWest,
}

impl CursorIcon {
    pub const ALL: [CursorIcon; 35] = [
        CursorIcon::Default,
        CursorIcon::ContextMenu,
        CursorIcon::Help,
        CursorIcon::PointingHand,
        CursorIcon::Progress,
        CursorIcon::Wait,
        CursorIcon::Cell,
        CursorIcon::Crosshair,
        CursorIcon::Text,
        CursorIcon::VerticalText,
        CursorIcon::Alias,
        CursorIcon::Copy,
        CursorIcon::Move,
        CursorIcon::NoDrop,
        CursorIcon::NotAllowed,
        CursorIcon::Grab,
        CursorIcon::Grabbing,
        CursorIcon::ResizeEast,
        CursorIcon::ResizeNorth,
        CursorIcon::ResizeNorthEast,
        CursorIcon::ResizeNorthWest,
        CursorIcon::ResizeSouth,
        CursorIcon::ResizeSouthEast,
        CursorIcon::ResizeSouthWest,
        CursorIcon::ResizeWest,
        CursorIcon::ResizeHorizontal,
        CursorIcon::ResizeVertical,
        CursorIcon::ResizeNeSw,
        CursorIcon::ResizeNwSe,
        CursorIcon::ResizeColumn,
        CursorIcon::ResizeRow,
        CursorIcon::AllScroll,
        CursorIcon::ZoomIn,
        CursorIcon::ZoomOut,
        CursorIcon::None,
    ];

    pub fn name(self) -> &'static str {
        match self {
            CursorIcon::Default => "default",
            CursorIcon::ContextMenu => "context-menu",
            CursorIcon::Help => "help",
            CursorIcon::PointingHand => "pointer",
            CursorIcon::Progress => "progress",
            CursorIcon::Wait => "wait",
            CursorIcon::Cell => "cell",
            CursorIcon::Crosshair => "crosshair",
            CursorIcon::Text => "text",
            CursorIcon::VerticalText => "vertical-text",
            CursorIcon::Alias => "alias",
            CursorIcon::Copy => "copy",
            CursorIcon::Move => "move",
            CursorIcon::NoDrop => "no-drop",
            CursorIcon::NotAllowed => "not-allowed",
            CursorIcon::Grab => "grab",
            CursorIcon::Grabbing => "grabbing",
            CursorIcon::ResizeEast => "e-resize",
            CursorIcon::ResizeNorth => "n-resize",
            CursorIcon::ResizeNorthEast => "ne-resize",
            CursorIcon::ResizeNorthWest => "nw-resize",
            CursorIcon::ResizeSouth => "s-resize",
            CursorIcon::ResizeSouthEast => "se-resize",
            CursorIcon::ResizeSouthWest => "sw-resize",
            CursorIcon::ResizeWest => "w-resize",
            CursorIcon::ResizeHorizontal => "ew-resize",
            CursorIcon::ResizeVertical => "ns-resize",
            CursorIcon::ResizeNeSw => "nesw-resize",
            CursorIcon::ResizeNwSe => "nwse-resize",
            CursorIcon::ResizeColumn => "col-resize",
            CursorIcon::ResizeRow => "row-resize",
            CursorIcon::AllScroll => "all-scroll",
            CursorIcon::ZoomIn => "zoom-in",
            CursorIcon::ZoomOut => "zoom-out",
            CursorIcon::None => "none",
        }
    }

    pub fn from_name(name: &str) -> Option<CursorIcon> {
        Self::ALL.into_iter().find(|icon| icon.name() == name)
    }
}

#[derive(Clone, Default, Debug)]
pub struct RawInput {
    pub events: Vec<Event>,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum PointerId {
    Mouse,
    Touch(TouchId),
}

impl PointerId {
    pub fn is_touch(self) -> bool {
        matches!(self, Self::Touch(_))
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PointerPhase {
    Down(PointerButton),
    Move,
    Up(PointerButton),
    Cancel,
    Gone,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct PointerEvent {
    pub pointer: PointerId,
    pub phase: PointerPhase,
    pub pos: Pos2,
    pub modifiers: Modifiers,
    pub clicks: u32,
    pub force: Option<f32>,
    pub source: usize,
}

#[derive(Clone, Debug)]
pub struct InputState {
    pub events: Vec<Event>,
    pub pointer_events: Vec<PointerEvent>,
    pub pointer: Pointer,
    pub scroll_delta: Vec2,
    pub scroll_fling: Vec2,
    pub zoom_factor: f32,
    pub modifiers: Modifiers,
    pub long_press_delay: Duration,
    touches: Vec<(TouchId, Pos2)>,
    source: usize,
    force: Option<f32>,
    scroll_samples: VecDeque<(Instant, Vec2)>,
}

impl Default for InputState {
    fn default() -> Self {
        Self {
            events: Vec::new(),
            pointer_events: Vec::new(),
            pointer: Pointer::default(),
            scroll_delta: Vec2::ZERO,
            scroll_fling: Vec2::ZERO,
            zoom_factor: 1.0,
            modifiers: Modifiers::default(),
            long_press_delay: LONG_PRESS_DELAY,
            touches: Vec::new(),
            source: 0,
            force: None,
            scroll_samples: VecDeque::new(),
        }
    }
}

impl InputState {
    pub fn scaled(&self, factor: f32) -> Self {
        let scale = |pos: Pos2| Pos2::new(pos.x * factor, pos.y * factor);
        let mut input = self.clone();
        for event in &mut input.events {
            match event {
                Event::PointerMoved(pos)
                | Event::PointerButton { pos, .. }
                | Event::Touch { pos, .. } => *pos = scale(*pos),
                _ => {}
            }
        }
        for event in &mut input.pointer_events {
            event.pos = scale(event.pos);
        }
        for (_, pos) in &mut input.touches {
            *pos = scale(*pos);
        }
        input.pointer.pos = input.pointer.pos.map(scale);
        input.pointer.press_pos = input.pointer.press_pos.map(scale);
        input.pointer.motion *= factor;
        input.pointer.last_click = input
            .pointer
            .last_click
            .map(|(when, pos)| (when, scale(pos)));
        input
    }

    pub fn touching(&self) -> bool {
        !self.touches.is_empty()
    }

    pub fn begin_frame(&mut self, raw: RawInput, now: Instant) {
        self.pointer.begin_frame();
        if self.touches.is_empty() && self.pointer.from_touch {
            self.pointer.pos = None;
            self.pointer.from_touch = false;
        }
        self.pointer_events.clear();
        self.scroll_delta = Vec2::ZERO;
        self.scroll_fling = Vec2::ZERO;
        self.zoom_factor = 1.0;
        let mut scroll_ended = false;
        let suppress_mouse = self.pointer.from_touch
            || raw
                .events
                .iter()
                .any(|event| matches!(event, Event::Touch { .. }));
        for (source, event) in raw.events.iter().enumerate() {
            self.source = source;
            self.force = None;
            match event {
                Event::PointerMoved(pos) if !suppress_mouse => {
                    self.pointer.pos = Some(*pos);
                    self.emit(PointerId::Mouse, PointerPhase::Move, *pos);
                }
                Event::PointerMotion(delta) => {
                    self.pointer.motion += *delta;
                }
                Event::PointerGone if !suppress_mouse => self.mouse_gone(),
                Event::Focus(false) => {
                    for (id, pos) in std::mem::take(&mut self.touches) {
                        self.emit(PointerId::Touch(id), PointerPhase::Cancel, pos);
                    }
                    if self.pointer.from_touch {
                        self.pointer.from_touch = false;
                        self.pointer.pos = None;
                        self.pointer.primary_down = false;
                    }
                    self.mouse_gone();
                }
                Event::PointerButton {
                    pos,
                    button,
                    pressed,
                    modifiers,
                } if !suppress_mouse => {
                    self.pointer.pos = Some(*pos);
                    self.modifiers = *modifiers;
                    self.pointer.button(*button, *pressed, *pos, now);
                    let phase = match pressed {
                        true => PointerPhase::Down(*button),
                        false => PointerPhase::Up(*button),
                    };
                    self.emit(PointerId::Mouse, phase, *pos);
                }
                Event::Scroll(delta) => self.scroll_delta += *delta,
                Event::ScrollEnded => scroll_ended = true,
                Event::Zoom(factor) => self.zoom_factor *= *factor,
                Event::Key { press, .. } => self.modifiers = press.modifiers,
                Event::Modifiers(modifiers) => self.modifiers = *modifiers,
                Event::Touch {
                    id,
                    phase,
                    pos,
                    force,
                } => {
                    self.force = *force;
                    self.touch(*id, *phase, *pos, now);
                }
                _ => {}
            }
        }
        self.events = raw.events;
        self.sample_scroll(now, scroll_ended);
    }

    fn emit(&mut self, pointer: PointerId, phase: PointerPhase, pos: Pos2) {
        self.pointer_events.push(PointerEvent {
            pointer,
            phase,
            pos,
            modifiers: self.modifiers,
            clicks: self.pointer.clicks,
            force: self.force,
            source: self.source,
        });
    }

    fn mouse_gone(&mut self) {
        if let Some(pos) = self.pointer.pos.filter(|_| !self.pointer.from_touch) {
            self.emit(PointerId::Mouse, PointerPhase::Gone, pos);
        }
        if !self.pointer.from_touch {
            self.pointer.pos = None;
            self.pointer.primary_down = false;
        }
        self.pointer.secondary_down = false;
        self.pointer.middle_down = false;
    }

    fn touch(&mut self, id: TouchId, phase: TouchPhase, pos: Pos2, now: Instant) {
        let held = self.touches.iter().position(|(finger, _)| *finger == id);
        let first = self.touches.first().map(|(finger, _)| *finger);
        match (phase, held) {
            (TouchPhase::Start, None) => {
                if self.touches.is_empty() {
                    let pointer = &mut self.pointer;
                    pointer.pos = Some(pos);
                    pointer.primary_down = true;
                    pointer.primary_pressed = true;
                    pointer.press_pos = Some(pos);
                    pointer.from_touch = true;
                    pointer.count_click(pos, now);
                }
                self.touches.push((id, pos));
                self.emit(
                    PointerId::Touch(id),
                    PointerPhase::Down(PointerButton::Primary),
                    pos,
                );
            }
            (TouchPhase::Move, Some(index)) => {
                self.touches[index].1 = pos;
                if first == Some(id) {
                    self.pointer.pos = Some(pos);
                }
                self.emit(PointerId::Touch(id), PointerPhase::Move, pos);
            }
            (TouchPhase::End | TouchPhase::Cancel, Some(index)) => {
                self.touches.remove(index);
                if first == Some(id) {
                    self.pointer.primary_down = false;
                    self.pointer.primary_released = true;
                    self.pointer.pos = Some(pos);
                    if phase == TouchPhase::Cancel {
                        self.pointer.pos = None;
                    }
                }
                let phase = match phase {
                    TouchPhase::End => PointerPhase::Up(PointerButton::Primary),
                    _ => PointerPhase::Cancel,
                };
                self.emit(PointerId::Touch(id), phase, pos);
            }
            _ => {}
        }
    }

    fn sample_scroll(&mut self, now: Instant, ended: bool) {
        if self.scroll_delta != Vec2::ZERO {
            self.scroll_samples.push_back((now, self.scroll_delta));
        }
        while self
            .scroll_samples
            .front()
            .is_some_and(|(when, _)| now.duration_since(*when) > WHEEL_VELOCITY_WINDOW)
        {
            self.scroll_samples.pop_front();
        }
        if !ended {
            return;
        }
        let samples = std::mem::take(&mut self.scroll_samples);
        let (Some((first, _)), Some((last, _))) = (samples.front(), samples.back()) else {
            return;
        };
        let elapsed = last.duration_since(*first).as_secs_f32();
        if elapsed <= f32::EPSILON {
            return;
        }
        let travelled = samples
            .iter()
            .skip(1)
            .fold(Vec2::ZERO, |sum, (_, delta)| sum + *delta);
        self.scroll_fling = travelled * elapsed.recip();
    }
}

#[derive(Clone, Copy, Default, Debug)]
pub struct Pointer {
    pub pos: Option<Pos2>,
    pub press_pos: Option<Pos2>,
    pub motion: Vec2,
    pub primary_down: bool,
    pub primary_pressed: bool,
    pub primary_released: bool,
    pub secondary_down: bool,
    pub secondary_pressed: bool,
    pub secondary_released: bool,
    pub middle_down: bool,
    pub middle_pressed: bool,
    pub middle_released: bool,
    clicks: u32,
    last_click: Option<(Instant, Pos2)>,
    from_touch: bool,
}

impl Pointer {
    fn begin_frame(&mut self) {
        self.primary_pressed = false;
        self.primary_released = false;
        self.secondary_pressed = false;
        self.secondary_released = false;
        self.middle_pressed = false;
        self.middle_released = false;
        self.motion = Vec2::ZERO;
    }

    fn button(&mut self, button: PointerButton, pressed: bool, pos: Pos2, now: Instant) {
        match button {
            PointerButton::Primary => {
                self.primary_down = pressed;
                if pressed {
                    self.primary_pressed = true;
                    self.press_pos = Some(pos);
                    self.count_click(pos, now);
                } else {
                    self.primary_released = true;
                }
            }
            PointerButton::Secondary => {
                self.secondary_down = pressed;
                match pressed {
                    true => self.secondary_pressed = true,
                    false => self.secondary_released = true,
                }
            }
            PointerButton::Middle => {
                self.middle_down = pressed;
                match pressed {
                    true => self.middle_pressed = true,
                    false => self.middle_released = true,
                }
            }
            PointerButton::Back | PointerButton::Forward => {}
        }
    }

    fn count_click(&mut self, pos: Pos2, now: Instant) {
        let repeated = self.last_click.is_some_and(|(when, at)| {
            now.duration_since(when).as_secs_f32() <= MULTI_CLICK_DELAY
                && at.distance(pos) <= MULTI_CLICK_DISTANCE
        });
        self.clicks = if repeated {
            self.clicks % MULTI_CLICK_LIMIT + 1
        } else {
            1
        };
        self.last_click = Some((now, pos));
    }

    pub fn clicks(&self) -> u32 {
        self.clicks
    }

    pub fn interact_pos(&self) -> Option<Pos2> {
        self.pos
    }

    pub fn from_touch(&self) -> bool {
        self.from_touch
    }

    pub fn primary_pressed(&self) -> bool {
        self.primary_pressed
    }

    pub fn primary_released(&self) -> bool {
        self.primary_released
    }

    pub fn secondary_pressed(&self) -> bool {
        self.secondary_pressed
    }

    pub fn secondary_released(&self) -> bool {
        self.secondary_released
    }

    pub fn middle_pressed(&self) -> bool {
        self.middle_pressed
    }

    pub fn middle_released(&self) -> bool {
        self.middle_released
    }
}

#[cfg(test)]
mod tests;
