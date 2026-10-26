use std::time::Instant;

use geometry::{Pos2, Vec2};

pub use ui_types::input::{
    BackEdge, BackGesture, CursorIcon, ImeEvent, ImeText, Input, Key, KeyChord, KeyPress, Keyboard,
    Keystroke, Modifiers, Pen, PenPhase, PointerButton, ScrollDelta, TouchId, TouchPhase,
};

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
