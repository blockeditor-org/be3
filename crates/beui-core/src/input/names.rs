use super::{Key, Modifiers};

impl Key {
    pub fn meta(apple: bool) -> Self {
        match apple {
            true => Self::Ctrl,
            false => Self::Logo,
        }
    }

    pub fn from_code(code: &str) -> Option<Self> {
        let key = match code {
            "ArrowDown" => Self::ArrowDown,
            "ArrowLeft" => Self::ArrowLeft,
            "ArrowRight" => Self::ArrowRight,
            "ArrowUp" => Self::ArrowUp,
            "Backspace" => Self::Backspace,
            "Delete" => Self::Delete,
            "End" => Self::End,
            "Enter" | "NumpadEnter" => Self::Enter,
            "Escape" => Self::Escape,
            "Home" => Self::Home,
            "BracketLeft" => Self::BracketLeft,
            "BracketRight" => Self::BracketRight,
            "Minus" | "NumpadSubtract" => Self::Minus,
            "PageDown" => Self::PageDown,
            "PageUp" => Self::PageUp,
            "Equal" | "NumpadAdd" => Self::Plus,
            "Space" => Self::Space,
            "Tab" => Self::Tab,
            "Digit0" | "Numpad0" => Self::Zero,
            "Digit1" | "Numpad1" => Self::One,
            "Digit2" | "Numpad2" => Self::Two,
            "Digit3" | "Numpad3" => Self::Three,
            "Digit4" | "Numpad4" => Self::Four,
            "Digit5" | "Numpad5" => Self::Five,
            "Digit6" | "Numpad6" => Self::Six,
            "Digit7" | "Numpad7" => Self::Seven,
            "Digit8" | "Numpad8" => Self::Eight,
            "Digit9" | "Numpad9" => Self::Nine,
            "Backquote" => Self::Backtick,
            "Insert" => Self::Insert,
            "Comma" => Self::Comma,
            "Period" | "NumpadDecimal" => Self::Period,
            "Slash" | "NumpadDivide" => Self::Slash,
            "Backslash" => Self::Backslash,
            "Semicolon" => Self::Semicolon,
            "Quote" => Self::Quote,
            "BrowserBack" => Self::BrowserBack,
            "AudioVolumeUp" => Self::VolumeUp,
            "AudioVolumeDown" => Self::VolumeDown,
            "AudioVolumeMute" => Self::VolumeMute,
            "MediaPlayPause" => Self::MediaPlayPause,
            "MediaTrackNext" => Self::MediaNext,
            "MediaTrackPrevious" => Self::MediaPrevious,
            "MediaStop" => Self::MediaStop,
            "ShiftLeft" | "ShiftRight" => Self::Shift,
            "ControlLeft" | "ControlRight" => Self::Ctrl,
            "AltLeft" | "AltRight" => Self::Alt,
            _ => return Self::letter_code(code).or_else(|| Self::function_code(code)),
        };
        Some(key)
    }

    fn letter_code(code: &str) -> Option<Self> {
        let letter = code.strip_prefix("Key")?;
        let mut chars = letter.chars();
        match (chars.next(), chars.next()) {
            (Some(letter @ 'A'..='Z'), None) => Self::from_char(letter),
            _ => None,
        }
    }

    fn function_code(code: &str) -> Option<Self> {
        Self::function(code.strip_prefix('F')?.parse().ok()?)
    }

    pub fn function(number: usize) -> Option<Self> {
        number
            .checked_sub(1)
            .and_then(|index| FUNCTION.get(index))
            .copied()
    }

    pub fn from_char(c: char) -> Option<Self> {
        let key = match c.to_ascii_lowercase() {
            '[' | '{' => Self::BracketLeft,
            ']' | '}' => Self::BracketRight,
            '-' | '_' => Self::Minus,
            '=' | '+' => Self::Plus,
            ' ' => Self::Space,
            '0' | ')' => Self::Zero,
            '1' | '!' => Self::One,
            '2' | '@' => Self::Two,
            '3' | '#' => Self::Three,
            '4' | '$' => Self::Four,
            '5' | '%' => Self::Five,
            '6' | '^' => Self::Six,
            '7' | '&' => Self::Seven,
            '8' | '*' => Self::Eight,
            '9' | '(' => Self::Nine,
            '`' | '~' => Self::Backtick,
            ',' | '<' => Self::Comma,
            '.' | '>' => Self::Period,
            '/' | '?' => Self::Slash,
            '\\' | '|' => Self::Backslash,
            ';' | ':' => Self::Semicolon,
            '\'' | '"' => Self::Quote,
            letter @ 'a'..='z' => LETTERS[usize::from(letter as u8 - b'a')],
            _ => return None,
        };
        Some(key)
    }
}

impl Modifiers {
    pub fn with_meta(alt: bool, ctrl: bool, shift: bool, meta: bool, apple: bool) -> Self {
        Self {
            alt,
            ctrl: ctrl || (apple && meta),
            shift,
            logo: !apple && meta,
        }
    }
}

const FUNCTION: [Key; 24] = [
    Key::F1,
    Key::F2,
    Key::F3,
    Key::F4,
    Key::F5,
    Key::F6,
    Key::F7,
    Key::F8,
    Key::F9,
    Key::F10,
    Key::F11,
    Key::F12,
    Key::F13,
    Key::F14,
    Key::F15,
    Key::F16,
    Key::F17,
    Key::F18,
    Key::F19,
    Key::F20,
    Key::F21,
    Key::F22,
    Key::F23,
    Key::F24,
];

const LETTERS: [Key; 26] = [
    Key::A,
    Key::B,
    Key::C,
    Key::D,
    Key::E,
    Key::F,
    Key::G,
    Key::H,
    Key::I,
    Key::J,
    Key::K,
    Key::L,
    Key::M,
    Key::N,
    Key::O,
    Key::P,
    Key::Q,
    Key::R,
    Key::S,
    Key::T,
    Key::U,
    Key::V,
    Key::W,
    Key::X,
    Key::Y,
    Key::Z,
];
