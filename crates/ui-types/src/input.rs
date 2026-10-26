pub use crate::types::{
    BackEdge, BackGesture, CursorIcon, ImeEvent, ImeText, Input, Key, KeyChord, KeyPress, Keyboard,
    Keystroke, Modifiers, Pen, PenPhase, PointerButton, ScrollDelta, TouchId, TouchPhase,
};

use geometry::Vec2;

mod names;

impl Key {
    pub const ALL: [Self; 102] = [
        Self::ArrowDown,
        Self::ArrowLeft,
        Self::ArrowRight,
        Self::ArrowUp,
        Self::Backspace,
        Self::BracketLeft,
        Self::BracketRight,
        Self::Delete,
        Self::End,
        Self::Enter,
        Self::Escape,
        Self::Home,
        Self::Minus,
        Self::PageDown,
        Self::PageUp,
        Self::Plus,
        Self::Space,
        Self::Tab,
        Self::Zero,
        Self::One,
        Self::Two,
        Self::Three,
        Self::Four,
        Self::Five,
        Self::Six,
        Self::Seven,
        Self::Eight,
        Self::Nine,
        Self::Backtick,
        Self::A,
        Self::B,
        Self::C,
        Self::D,
        Self::E,
        Self::F,
        Self::G,
        Self::H,
        Self::I,
        Self::J,
        Self::K,
        Self::L,
        Self::M,
        Self::N,
        Self::O,
        Self::P,
        Self::Q,
        Self::R,
        Self::S,
        Self::T,
        Self::U,
        Self::V,
        Self::W,
        Self::X,
        Self::Y,
        Self::Z,
        Self::Insert,
        Self::Comma,
        Self::Period,
        Self::Slash,
        Self::Backslash,
        Self::Semicolon,
        Self::Quote,
        Self::BrowserBack,
        Self::F1,
        Self::F2,
        Self::F3,
        Self::F4,
        Self::F5,
        Self::F6,
        Self::F7,
        Self::F8,
        Self::F9,
        Self::F10,
        Self::F11,
        Self::F12,
        Self::F13,
        Self::F14,
        Self::F15,
        Self::F16,
        Self::F17,
        Self::F18,
        Self::F19,
        Self::F20,
        Self::F21,
        Self::F22,
        Self::F23,
        Self::F24,
        Self::VolumeUp,
        Self::VolumeDown,
        Self::VolumeMute,
        Self::MicMute,
        Self::BrightnessUp,
        Self::BrightnessDown,
        Self::MediaPlayPause,
        Self::MediaNext,
        Self::MediaPrevious,
        Self::MediaStop,
        Self::Shift,
        Self::Ctrl,
        Self::Alt,
        Self::Logo,
        Self::Unidentified,
    ];

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
}
