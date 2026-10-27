use std::time::{Duration, Instant};

use geometry::{Pos2, Vec2};

use crate::layout::Direction;
use beui_tree::reactive::{IntoProp, Memo, Prop, ReadSignal, Signal};

pub use ui_types::input::{
    BackEdge, BackGesture, CursorIcon, ImeEvent, ImeText, Input, Key, KeyChord, KeyPress, Keyboard,
    Keystroke, Modifiers, Pen, PenPhase, PointerButton, ScrollDelta, TouchId, TouchPhase,
};

pub mod typing;

pub const TOUCH_DRAG_THRESHOLD: f32 = 8.0;
pub const WHEEL_LATCH_TIMEOUT: Duration = Duration::from_millis(500);
pub const TOUCH_VELOCITY_WINDOW: f32 = 0.12;
pub const MAX_TOUCH_VELOCITY: f32 = 4_000.0;

const MULTI_CLICK_DELAY: f32 = 0.3;
const MULTI_CLICK_DISTANCE: f32 = 6.0;
const MULTI_CLICK_LIMIT: u32 = 4;

#[derive(Clone, Copy, Default, Debug)]
pub struct ClickCounter {
    clicks: u32,
    last: Option<(Instant, Pos2)>,
}

impl ClickCounter {
    pub fn count(&mut self, pos: Pos2, now: Instant) -> u32 {
        let repeated = self.last.is_some_and(|(when, at)| {
            now.saturating_duration_since(when).as_secs_f32() <= MULTI_CLICK_DELAY
                && at.distance(pos) <= MULTI_CLICK_DISTANCE
        });
        self.clicks = match repeated {
            true => self.clicks % MULTI_CLICK_LIMIT + 1,
            false => 1,
        };
        self.last = Some((now, pos));
        self.clicks
    }

    pub fn clicks(&self) -> u32 {
        self.clicks
    }

    pub fn scale(&mut self, scale: impl Fn(Pos2) -> Pos2) {
        self.last = self.last.map(|(when, pos)| (when, scale(pos)));
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum PointerId {
    Mouse,
    Touch(TouchId),
    Pen,
}

pub const CONTROLLER_TOUCH_DEVICE: u64 = 0x1a5e_0000;

impl PointerId {
    pub fn is_touch(self) -> bool {
        matches!(self, Self::Touch(_))
    }

    pub fn is_controller(self) -> bool {
        matches!(self, Self::Touch(id) if id.device == CONTROLLER_TOUCH_DEVICE)
    }

    pub fn hovers(self) -> bool {
        !self.is_touch()
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum TouchDrags {
    #[default]
    None,
    Along(Direction),
    All,
}

impl TouchDrags {
    pub fn when(on: bool) -> Self {
        match on {
            true => TouchDrags::All,
            false => TouchDrags::None,
        }
    }

    pub fn holds(self, direction: Direction) -> bool {
        match self {
            TouchDrags::None => false,
            TouchDrags::Along(along) => along == direction,
            TouchDrags::All => true,
        }
    }
}

impl IntoProp<TouchDrags> for bool {
    fn into_prop(self) -> Prop<TouchDrags> {
        Prop::Static(TouchDrags::when(self))
    }
}

impl IntoProp<TouchDrags> for Prop<bool> {
    fn into_prop(self) -> Prop<TouchDrags> {
        self.map(TouchDrags::when)
    }
}

impl IntoProp<TouchDrags> for Memo<bool> {
    fn into_prop(self) -> Prop<TouchDrags> {
        IntoProp::<bool>::into_prop(self).map(TouchDrags::when)
    }
}

impl IntoProp<TouchDrags> for Signal<bool> {
    fn into_prop(self) -> Prop<TouchDrags> {
        IntoProp::<bool>::into_prop(self).map(TouchDrags::when)
    }
}

impl IntoProp<TouchDrags> for ReadSignal<bool> {
    fn into_prop(self) -> Prop<TouchDrags> {
        IntoProp::<bool>::into_prop(self).map(TouchDrags::when)
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum SoftKeyboard {
    #[default]
    OnFocus,
    OnPress,
    Held,
    Shown {
        asks: u32,
    },
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PointerStage {
    Down,
    Move,
    Up,
    Cancel,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct PointerInput {
    pub pointer: PointerId,
    pub button: Option<PointerButton>,
    pub stage: PointerStage,
    pub pos: Pos2,
    pub local: Pos2,
    pub size: Vec2,
    pub scale: f32,
    pub inside: bool,
    pub modifiers: Modifiers,
    pub time: Instant,
    pub settles: Option<Direction>,
    pub unclaimed: bool,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum WheelInput {
    Scroll(ScrollGesture),
    Zoom(ZoomGesture),
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct PointerPress {
    pub window: ui_types::window::WindowId,
    pub pos: Pos2,
    pub local: Pos2,
    pub size: Vec2,
    pub fraction: Vec2,
    pub clicks: u32,
    pub modifiers: Modifiers,
    pub touch: bool,
    pub pen: Option<Pen>,
    pub predicted: Option<Pos2>,
    pub time: Instant,
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
    pub local: Pos2,
    pub size: Vec2,
    pub modifiers: Modifiers,
    pub held: bool,
    pub unclaimed: bool,
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
    pub local: Pos2,
    pub size: Vec2,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KeyPhase {
    Capture,
    Bubble,
    Held,
}

pub const SCROLL_LINE: f32 = 40.0;
pub const SCROLL_PAGE: f32 = 800.0;
pub const PINCH_SPEED: f32 = 0.01;

pub fn scrolled(delta: ScrollDelta, scale: f32) -> Vec2 {
    match delta {
        ScrollDelta::Pixels(delta) => delta / scale,
        ScrollDelta::Lines(delta) => delta * SCROLL_LINE,
        ScrollDelta::Pages(delta) => delta * SCROLL_PAGE,
    }
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct DroppedFile {
    pub name: String,
    pub path: Option<std::path::PathBuf>,
    pub bytes: Option<std::sync::Arc<[u8]>>,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum FileDrag {
    Hovered,
    Cancelled,
    Dropped(DroppedFile),
}

#[derive(Clone, PartialEq, Debug)]
pub enum Event {
    Focus(bool),
    Modifiers(Modifiers),
    Key {
        press: KeyPress,
        evdev: Option<u32>,
        text: Option<String>,
    },
    PointerButton {
        pos: Pos2,
        button: PointerButton,
        pressed: bool,
        modifiers: Modifiers,
    },
    PointerGone,
    PointersCancelled,
    PointerMotion(Vec2),
    PointerUnlocked,
    PointerMoved(Pos2),
    ScrollStarted,
    Scroll(Vec2),
    ScrollEnded,
    Text(String),
    Paste(ui_types::clipboard::Pasted),
    Touch {
        id: TouchId,
        phase: TouchPhase,
        pos: Pos2,
        force: Option<f32>,
    },
    Pen {
        phase: PenPhase,
        pos: Pos2,
        pen: Pen,
    },
    Predicted(Pos2),
    Time(Instant),
    Zoom(f32),
    Ime(ImeEvent),
    FileHovered,
    FileHoverCancelled,
    FileDropped(DroppedFile),
    Back(BackGesture),
    InterceptedKey(KeyPress),
    InterceptedTap(Key),
    PointerWindow(crate::window::WindowId),
    PointerOutside,
}

impl Event {
    pub fn key(press: KeyPress) -> Self {
        Self::Key {
            press,
            evdev: None,
            text: None,
        }
    }

    pub fn press(&self) -> Option<KeyPress> {
        match self {
            Self::Key { press, .. } => Some(*press),
            _ => None,
        }
    }

    pub fn is_pointer(&self) -> bool {
        matches!(
            self,
            Self::PointerButton { .. }
                | Self::PointerGone
                | Self::PointersCancelled
                | Self::PointerMotion(_)
                | Self::PointerMoved(_)
                | Self::ScrollStarted
                | Self::Scroll(_)
                | Self::ScrollEnded
                | Self::Touch { .. }
                | Self::Pen { .. }
                | Self::Predicted(_)
                | Self::Time(_)
                | Self::Zoom(_)
                | Self::PointerWindow(_)
                | Self::PointerOutside
        )
    }

    pub fn is_key(&self) -> bool {
        matches!(
            self,
            Self::Key { .. }
                | Self::Text(_)
                | Self::Ime(_)
                | Self::Modifiers(_)
                | Self::InterceptedKey(_)
                | Self::InterceptedTap(_)
        )
    }

    pub fn modifiers(&self) -> Option<Modifiers> {
        match self {
            Self::Key { press, .. } | Self::InterceptedKey(press) => Some(press.modifiers),
            Self::Modifiers(modifiers) | Self::PointerButton { modifiers, .. } => Some(*modifiers),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum PointerPhase {
    Down(PointerButton),
    Move,
    Up(PointerButton),
    Cancel,
    Gone,
    ScrollStarted,
    Scroll(Vec2),
    ScrollEnded { fling: Vec2 },
    Zoom(f32),
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct PointerEvent {
    pub pointer: PointerId,
    pub window: crate::window::WindowId,
    pub phase: PointerPhase,
    pub pos: Pos2,
    pub modifiers: Modifiers,
    pub clicks: u32,
    pub force: Option<f32>,
    pub pen: Option<Pen>,
    pub predicted: Option<Pos2>,
    pub time: Instant,
    pub source: usize,
}
