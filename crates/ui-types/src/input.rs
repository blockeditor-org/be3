pub use crate::types::{
    BackEdge, BackGesture, CursorIcon, ImeEvent, ImeText, Input, Keyboard, Keystroke, Pen,
    PenPhase, PointerButton, ScrollDelta, TouchId, TouchPhase,
};
pub use ui_keys::{Key, KeyChord, KeyPress, Modifiers};

pub use queue::{Queued, enqueue};

use geometry::{Pos2, Vec2};

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
            CursorIcon::Touch => "crosshair",
        }
    }

    pub fn from_name(name: &str) -> Option<CursorIcon> {
        Self::ALL.into_iter().find(|icon| icon.name() == name)
    }
}

#[cfg(test)]
mod tests;

impl Input {
    pub fn translated(mut self, by: Vec2) -> Self {
        match &mut self {
            Self::PointerMoved(pos)
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
            | Self::Predicted(pos)
            | Self::PointerButton { pos, .. }
            | Self::Touch { pos, .. }
            | Self::Pen { pos, .. } => *pos = Pos2::new(pos.x * factor, pos.y * factor),
            Self::PointerMotion(by) | Self::Scroll(ScrollDelta::Pixels(by)) => *by *= factor,
            _ => {}
        }
        self
    }
}
