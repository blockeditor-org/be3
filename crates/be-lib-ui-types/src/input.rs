pub use crate::types::{
    BackEdge, BackGesture, CursorIcon, ImeEvent, ImeText, Input, Keyboard, Keystroke, Pen,
    PenPhase, PointerButton, ScrollDelta, TouchId, TouchPhase, TouchpadGesture,
};
pub use be_lib_ui_keys::{Key, KeyChord, KeyPress, Modifiers, apple_keys, set_apple_keys};

pub use queue::{Queued, enqueue};

use be_lib_geometry::{Pos2, Vec2};

mod queue;

impl Keyboard {
    pub fn shown(self) -> bool {
        matches!(self, Self::Shown { .. })
    }
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

    /// Its CSS `cursor` keyword, but `Touch`'s, which CSS has none of (a page draws it as an
    /// image, `crosshair` its fallback): `touch`, so every cursor has a name of its own.
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
            CursorIcon::Touch => "touch",
        }
    }

    /// Its value for CSS's `cursor`: its name, but `Touch`'s fallback, `crosshair`, which a page
    /// that draws the touch cursor as an image falls back to.
    pub fn css(self) -> &'static str {
        match self {
            CursorIcon::Touch => CursorIcon::Crosshair.name(),
            cursor => cursor.name(),
        }
    }

    pub fn from_name(name: &str) -> Option<CursorIcon> {
        Self::ALL
            .into_iter()
            .chain([CursorIcon::Touch])
            .find(|icon| icon.name() == name)
    }
}

#[cfg(test)]
mod tests;

impl TouchpadGesture {
    pub fn scaled(mut self, factor: f32) -> Self {
        if let Self::Swiped(by) | Self::Pinched { delta: by, .. } = &mut self {
            *by *= factor;
        }
        self
    }
}

/// An input and when the platform sampled it, as a host hands a root guest its window's input.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Timed {
    pub input: Input,
    /// When it was sampled, in nanoseconds on the frame clock the host and its guest share
    /// (be3:root's `frame-request` `time`), if the platform said: pens and touches mostly do,
    /// so a stroke drawn from every sample keeps its pace. It is real time from the clock's
    /// origin, so while the clock is stopped (drive's) it can be later than the frame's time.
    pub time: Option<u64>,
}

impl From<Input> for Timed {
    fn from(input: Input) -> Self {
        Self { input, time: None }
    }
}

/// `events` as bytes, as a host hands a root guest its window's input (be3:root's
/// `input.deliver`, crates/be-wasm-root-api), which `decode` reads back.
pub fn encode(events: &[Timed]) -> Vec<u8> {
    postcard::to_allocvec(events).expect("input always serializes")
}

/// The events `encode` wrote.
pub fn decode(bytes: &[u8]) -> Result<Vec<Timed>, String> {
    postcard::from_bytes(bytes).map_err(|error| format!("the input could not be read: {error}"))
}

impl Input {
    pub fn translated(mut self, by: Vec2) -> Self {
        match &mut self {
            Self::PointerMoved(pos)
            | Self::PointerRested(pos)
            | Self::Predicted(pos)
            | Self::PointerButton { pos, .. }
            | Self::Touch { pos, .. }
            | Self::Pen { pos, .. } => *pos += by,
            _ => {}
        }
        self
    }

    pub fn scaled(mut self, factor: f32) -> Self {
        match &mut self {
            Self::PointerMoved(pos)
            | Self::PointerRested(pos)
            | Self::Predicted(pos)
            | Self::PointerButton { pos, .. }
            | Self::Touch { pos, .. }
            | Self::Pen { pos, .. } => *pos = Pos2::new(pos.x * factor, pos.y * factor),
            Self::PointerMotion(by) | Self::Scroll(ScrollDelta::Pixels(by)) => *by *= factor,
            Self::Touchpad(gesture) => *gesture = gesture.scaled(factor),
            _ => {}
        }
        self
    }
}
