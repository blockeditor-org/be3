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

impl Key {
    pub fn modifier_named(name: &str) -> Option<Self> {
        match name.to_ascii_lowercase().as_str() {
            "shift" => Some(Key::Shift),
            "ctrl" | "control" => Some(Key::Ctrl),
            "alt" => Some(Key::Alt),
            "super" | "logo" | "meta" | "win" => Some(Key::Logo),
            _ => None,
        }
    }

    pub fn from_name(name: &str) -> Option<Self> {
        if let Some(key) = Self::modifier_named(name) {
            return Some(key);
        }
        let lower = name.to_ascii_lowercase();
        let mut chars = name.chars();
        if let (Some(single), None) = (chars.next(), chars.next())
            && let Some(key) = Self::unshifted(single.to_ascii_lowercase())
        {
            return Some(key);
        }
        if let Some(number) = lower.strip_prefix('f')
            && let Ok(number) = number.parse::<usize>()
        {
            return Self::function(number);
        }
        Some(match lower.as_str() {
            "enter" | "return" => Key::Enter,
            "escape" | "esc" => Key::Escape,
            "tab" => Key::Tab,
            "backspace" => Key::Backspace,
            "delete" | "del" => Key::Delete,
            "insert" => Key::Insert,
            "space" => Key::Space,
            "up" | "arrowup" => Key::ArrowUp,
            "down" | "arrowdown" => Key::ArrowDown,
            "left" | "arrowleft" => Key::ArrowLeft,
            "right" | "arrowright" => Key::ArrowRight,
            "home" => Key::Home,
            "end" => Key::End,
            "pageup" => Key::PageUp,
            "pagedown" => Key::PageDown,
            "minus" => Key::Minus,
            "plus" | "equal" => Key::Plus,
            "comma" => Key::Comma,
            "period" => Key::Period,
            "slash" => Key::Slash,
            "backslash" => Key::Backslash,
            "semicolon" => Key::Semicolon,
            "quote" => Key::Quote,
            "backtick" | "grave" => Key::Backtick,
            "bracketleft" => Key::BracketLeft,
            "bracketright" => Key::BracketRight,
            "back" | "browserback" => Key::BrowserBack,
            "volumeup" => Key::VolumeUp,
            "volumedown" => Key::VolumeDown,
            "mute" | "volumemute" => Key::VolumeMute,
            "micmute" => Key::MicMute,
            "brightnessup" => Key::BrightnessUp,
            "brightnessdown" => Key::BrightnessDown,
            "play" | "playpause" => Key::MediaPlayPause,
            "next" => Key::MediaNext,
            "previous" => Key::MediaPrevious,
            "stop" => Key::MediaStop,
            _ => return None,
        })
    }

    pub fn unshifted(character: char) -> Option<Self> {
        Some(match character {
            'a'..='z' => LETTERS[character as usize - 'a' as usize],
            '0'..='9' => DIGITS[character as usize - '0' as usize],
            '-' => Key::Minus,
            '+' | '=' => Key::Plus,
            ',' => Key::Comma,
            '.' => Key::Period,
            '/' => Key::Slash,
            '\\' => Key::Backslash,
            ';' => Key::Semicolon,
            '\'' => Key::Quote,
            '`' => Key::Backtick,
            '[' => Key::BracketLeft,
            ']' => Key::BracketRight,
            ' ' => Key::Space,
            _ => return None,
        })
    }

    pub fn name(self) -> Option<String> {
        let debug = format!("{self:?}");
        if Self::from_name(&debug) == Some(self) {
            return Some(debug);
        }
        ('!'..='~')
            .find(|single| Self::unshifted(*single) == Some(self))
            .map(String::from)
    }

    pub fn evdev(self) -> Option<u32> {
        const LETTER_CODES: [u32; 26] = [
            30, 48, 46, 32, 18, 33, 34, 35, 23, 36, 37, 38, 50, 49, 24, 25, 16, 19, 31, 20, 22, 47,
            17, 45, 21, 44,
        ];
        if let Some(index) = LETTERS.iter().position(|letter| *letter == self) {
            return Some(LETTER_CODES[index]);
        }
        if let Some(index) = DIGITS.iter().position(|digit| *digit == self) {
            return Some(if index == 0 { 11 } else { index as u32 + 1 });
        }
        if let Some(index) = FUNCTION.iter().position(|function| *function == self) {
            return Some(match index {
                0..=9 => 59 + index as u32,
                10 => 87,
                11 => 88,
                _ => 183 + (index as u32 - 12),
            });
        }
        Some(match self {
            Key::Escape => 1,
            Key::Minus => 12,
            Key::Plus => 13,
            Key::Backspace => 14,
            Key::Tab => 15,
            Key::BracketLeft => 26,
            Key::BracketRight => 27,
            Key::Enter => 28,
            Key::Ctrl => 29,
            Key::Semicolon => 39,
            Key::Quote => 40,
            Key::Backtick => 41,
            Key::Shift => 42,
            Key::Backslash => 43,
            Key::Comma => 51,
            Key::Period => 52,
            Key::Slash => 53,
            Key::Alt => 56,
            Key::Space => 57,
            Key::Home => 102,
            Key::ArrowUp => 103,
            Key::PageUp => 104,
            Key::ArrowLeft => 105,
            Key::ArrowRight => 106,
            Key::End => 107,
            Key::ArrowDown => 108,
            Key::PageDown => 109,
            Key::Insert => 110,
            Key::Delete => 111,
            Key::VolumeMute => 113,
            Key::VolumeDown => 114,
            Key::VolumeUp => 115,
            Key::Logo => 125,
            Key::BrowserBack => 158,
            Key::MediaNext => 163,
            Key::MediaPlayPause => 164,
            Key::MediaPrevious => 165,
            Key::MediaStop => 166,
            Key::BrightnessDown => 224,
            Key::BrightnessUp => 225,
            Key::MicMute => 248,
            _ => return None,
        })
    }
}

const DIGITS: [Key; 10] = [
    Key::Zero,
    Key::One,
    Key::Two,
    Key::Three,
    Key::Four,
    Key::Five,
    Key::Six,
    Key::Seven,
    Key::Eight,
    Key::Nine,
];
