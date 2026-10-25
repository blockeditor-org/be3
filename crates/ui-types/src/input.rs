use std::ops::Range;

use geometry::{Pos2, Vec2};
use serde::{Deserialize, Serialize};

use crate::clipboard::Pasted;

mod names;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
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

#[derive(Clone, Copy, PartialEq, Eq, Default, Debug, Serialize, Deserialize)]
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

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct KeyPress {
    pub key: Key,
    pub pressed: bool,
    pub repeat: bool,
    pub modifiers: Modifiers,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
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

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum PointerButton {
    Primary,
    Secondary,
    Middle,
    Back,
    Forward,
    Other(u16),
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum PenPhase {
    Move,
    Down(PointerButton),
    Up(PointerButton),
    Gone,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum TouchPhase {
    Start,
    Move,
    End,
    Cancel,
}

#[derive(Clone, Copy, PartialEq, Debug, Serialize, Deserialize)]
pub enum BackGesture {
    Started { edge: BackEdge },
    Progressed(f32),
    Cancelled,
    Invoked,
}

#[derive(Clone, Copy, PartialEq, Eq, Default, Debug, Serialize, Deserialize)]
pub enum BackEdge {
    #[default]
    None,
    Left,
    Right,
}

#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum ImeEvent {
    Enabled,
    Disabled,
    SetComposingText(String),
    CommitText(String),
    FinishComposing,
    SetComposingRegion(Range<usize>),
    ReplaceText { range: Range<usize>, text: String },
    DeleteSurrounding { before: usize, after: usize },
    SetSelection { anchor: usize, focus: usize },
}

#[derive(Clone, PartialEq, Eq, Default, Debug, Serialize, Deserialize)]
pub struct ImeText {
    pub start: usize,
    pub text: String,
    pub selection: Range<usize>,
    pub composing: Option<Range<usize>>,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum Keyboard {
    Held,
    Shown { asks: u32 },
}

impl Keyboard {
    pub fn shown(self) -> bool {
        matches!(self, Self::Shown { .. })
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Default, Debug, Serialize, Deserialize)]
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
    Touch,
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

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub struct TouchId {
    pub device: u64,
    pub finger: u64,
}

#[derive(Clone, Copy, PartialEq, Default, Debug, Serialize, Deserialize)]
pub struct Pen {
    pub pressure: f32,
    pub tilt: Vec2,
    pub eraser: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Keystroke {
    pub key: Option<Key>,
    pub pressed: bool,
    pub repeat: bool,
    pub evdev: Option<u32>,
    pub text: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum ScrollDelta {
    Pixels(Vec2),
    Lines(Vec2),
    Pages(Vec2),
}

#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub enum Input {
    Focus(bool),
    Modifiers(Modifiers),
    Key(Keystroke),
    Text(String),
    Paste(Pasted),
    Ime(ImeEvent),
    PointerMoved(Pos2),
    PointerLeft,
    PointersCancelled,
    PointerMotion(Vec2),
    PointerUnlocked,
    PointerButton {
        pos: Pos2,
        button: PointerButton,
        pressed: bool,
    },
    ScrollStarted,
    Scroll(ScrollDelta),
    ScrollEnded,
    Zoom(f32),
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
    Back(BackGesture),
    InterceptedKey {
        key: Key,
        pressed: bool,
        repeat: bool,
    },
    InterceptedTap(Key),
}

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
