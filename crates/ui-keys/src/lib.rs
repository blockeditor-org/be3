be_ops_bindgen::generate!({
    path: "wit",
    world: "keys",
    default: ["modifiers"],
});

mod names;

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
}

#[cfg(test)]
mod tests;
