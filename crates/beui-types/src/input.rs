use std::time::Instant;

use geometry::{Pos2, Vec2};

use crate::layout::Direction;

pub use ui_types::input::{
    BackEdge, BackGesture, CursorIcon, ImeEvent, ImeText, Input, Key, KeyChord, KeyPress, Keyboard,
    Keystroke, Modifiers, Pen, PenPhase, PointerButton, ScrollDelta, TouchId, TouchPhase,
};

pub const TOUCH_DRAG_THRESHOLD: f32 = 8.0;
pub const TOUCH_VELOCITY_WINDOW: f32 = 0.12;
pub const MAX_TOUCH_VELOCITY: f32 = 4_000.0;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum PointerId {
    Mouse,
    Touch(TouchId),
    Pen,
}

impl PointerId {
    pub fn is_touch(self) -> bool {
        matches!(self, Self::Touch(_))
    }

    pub fn hovers(self) -> bool {
        !self.is_touch()
    }
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
