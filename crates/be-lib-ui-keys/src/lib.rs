be_wasm_ops_bindgen::generate!({
    path: "wit",
    world: "keys",
    default: ["modifiers"],
});

mod names;

use std::sync::atomic::{AtomicBool, Ordering};

pub use crate::types::{Key, KeyChord, KeyPress, Modifiers};

impl Key {
    pub const ALL: [Self; 103] = [
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
        Self::ContextMenu,
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

    /// How a menu or a screen reader names the chord on this platform:
    /// "Ctrl+Shift+P", "Super"; on Apple's systems, where Cmd is held as
    /// `ctrl`, "Cmd+Shift+P" and "Option" for `alt`.
    pub fn label(self) -> String {
        self.label_as(apple_keys())
    }

    fn label_as(self, apple: bool) -> String {
        let mut parts = Vec::new();
        if self.modifiers.logo {
            parts.push(key_label(Key::Logo, apple));
        }
        if self.modifiers.ctrl {
            parts.push(key_label(Key::Ctrl, apple));
        }
        if self.modifiers.alt {
            parts.push(key_label(Key::Alt, apple));
        }
        if self.modifiers.shift {
            parts.push(key_label(Key::Shift, apple));
        }
        parts.push(key_label(self.key, apple));
        parts.join("+")
    }
}

/// Whether keys follow Apple's systems; see [`set_apple_keys`].
static APPLE: AtomicBool = AtomicBool::new(cfg!(target_vendor = "apple"));

/// Makes keys follow Apple's systems from now on, or not: chords are named
/// with Cmd and Option, and controls take Apple's keys where they differ
/// (a text area's Cmd+Left goes to the line's start, not the word's). A build
/// for Apple's systems does from the start; a platform that learns where it
/// runs only when it starts, as a browser's does, says so here.
pub fn set_apple_keys(apple: bool) {
    APPLE.store(apple, Ordering::Relaxed);
}

/// Whether keys follow Apple's systems, where Command is held as `ctrl`; see
/// [`set_apple_keys`].
pub fn apple_keys() -> bool {
    APPLE.load(Ordering::Relaxed)
}

fn key_label(key: Key, apple: bool) -> String {
    let symbol = match key {
        Key::Ctrl if apple => "Cmd",
        Key::Alt if apple => "Option",
        Key::ArrowDown => "Down",
        Key::ArrowLeft => "Left",
        Key::ArrowRight => "Right",
        Key::ArrowUp => "Up",
        Key::BracketLeft => "[",
        Key::BracketRight => "]",
        Key::Minus => "-",
        Key::Plus => "+",
        Key::Zero => "0",
        Key::One => "1",
        Key::Two => "2",
        Key::Three => "3",
        Key::Four => "4",
        Key::Five => "5",
        Key::Six => "6",
        Key::Seven => "7",
        Key::Eight => "8",
        Key::Nine => "9",
        Key::Backtick => "`",
        Key::Comma => ",",
        Key::Period => ".",
        Key::Slash => "/",
        Key::Backslash => "\\",
        Key::Semicolon => ";",
        Key::Quote => "'",
        Key::PageDown => "Page Down",
        Key::PageUp => "Page Up",
        Key::BrowserBack => "Back",
        Key::ContextMenu => "Menu",
        Key::VolumeUp => "Volume Up",
        Key::VolumeDown => "Volume Down",
        Key::VolumeMute => "Mute",
        Key::MicMute => "Mic Mute",
        Key::BrightnessUp => "Brightness Up",
        Key::BrightnessDown => "Brightness Down",
        Key::MediaPlayPause => "Play/Pause",
        Key::MediaNext => "Next Track",
        Key::MediaPrevious => "Previous Track",
        Key::MediaStop => "Stop",
        Key::Logo => "Super",
        _ => return format!("{key:?}"),
    };
    symbol.to_owned()
}

#[cfg(test)]
mod tests;
