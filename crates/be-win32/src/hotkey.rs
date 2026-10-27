use std::fmt;

pub const MOD_ALT: u32 = 0x0001;
pub const MOD_CONTROL: u32 = 0x0002;
pub const MOD_SHIFT: u32 = 0x0004;
pub const MOD_WIN: u32 = 0x0008;
pub const MOD_NOREPEAT: u32 = 0x4000;

pub const ERROR_HOTKEY_ALREADY_REGISTERED: u32 = 1409;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Modifiers {
    pub control: bool,
    pub alt: bool,
    pub shift: bool,
    pub win: bool,
}

impl Modifiers {
    pub fn is_empty(&self) -> bool {
        !(self.control || self.alt || self.shift || self.win)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Hotkey {
    pub modifiers: Modifiers,
    pub key: u32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum HotkeyError {
    Taken(Hotkey),
    NoModifier,
    NoKey,
    UnknownKey(String),
    Failed(String),
}

impl fmt::Display for HotkeyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Taken(hotkey) => write!(f, "another program already uses {hotkey}"),
            Self::NoModifier => write!(
                f,
                "a hotkey needs Ctrl, Alt, Shift or Win, or it takes the key from every program"
            ),
            Self::NoKey => write!(f, "a hotkey needs a key besides its modifiers"),
            Self::UnknownKey(name) => write!(f, "no key is called {name:?}"),
            Self::Failed(message) => write!(f, "Windows refused the hotkey: {message}"),
        }
    }
}

impl std::error::Error for HotkeyError {}

const NAMED: [(&str, u32); 22] = [
    ("Space", 0x20),
    ("Tab", 0x09),
    ("Enter", 0x0D),
    ("Escape", 0x1B),
    ("Backspace", 0x08),
    ("Insert", 0x2D),
    ("Delete", 0x2E),
    ("Home", 0x24),
    ("End", 0x23),
    ("PageUp", 0x21),
    ("PageDown", 0x22),
    ("Left", 0x25),
    ("Up", 0x26),
    ("Right", 0x27),
    ("Down", 0x28),
    ("Pause", 0x13),
    ("ScrollLock", 0x91),
    ("`", 0xC0),
    ("-", 0xBD),
    ("=", 0xBB),
    (",", 0xBC),
    (".", 0xBE),
];

impl Hotkey {
    pub const DEFAULT: Self = Self {
        modifiers: Modifiers {
            control: true,
            alt: true,
            shift: false,
            win: false,
        },
        key: 0x20,
    };

    pub fn parse(text: &str) -> Result<Self, HotkeyError> {
        let mut modifiers = Modifiers::default();
        let mut key = None;
        for part in text.split('+').map(str::trim) {
            match part.to_ascii_lowercase().as_str() {
                "ctrl" | "control" => modifiers.control = true,
                "alt" => modifiers.alt = true,
                "shift" => modifiers.shift = true,
                "win" | "windows" | "super" => modifiers.win = true,
                "" => return Err(HotkeyError::NoKey),
                _ if key.is_some() => return Err(HotkeyError::UnknownKey(part.to_owned())),
                _ => {
                    key = Some(
                        key_code(part).ok_or_else(|| HotkeyError::UnknownKey(part.to_owned()))?,
                    )
                }
            }
        }
        let hotkey = Self {
            modifiers,
            key: key.ok_or(HotkeyError::NoKey)?,
        };
        hotkey.check()?;
        Ok(hotkey)
    }

    pub fn check(&self) -> Result<(), HotkeyError> {
        if self.modifiers.is_empty() {
            return Err(HotkeyError::NoModifier);
        }
        if self.key == 0 {
            return Err(HotkeyError::NoKey);
        }
        Ok(())
    }

    pub fn flags(&self) -> u32 {
        let Modifiers {
            control,
            alt,
            shift,
            win,
        } = self.modifiers;
        [
            (control, MOD_CONTROL),
            (alt, MOD_ALT),
            (shift, MOD_SHIFT),
            (win, MOD_WIN),
        ]
        .into_iter()
        .filter(|(on, _)| *on)
        .fold(MOD_NOREPEAT, |flags, (_, flag)| flags | flag)
    }

    pub fn registration_error(&self, code: u32, message: String) -> HotkeyError {
        if code == ERROR_HOTKEY_ALREADY_REGISTERED {
            HotkeyError::Taken(*self)
        } else {
            HotkeyError::Failed(message)
        }
    }
}

impl Default for Hotkey {
    fn default() -> Self {
        Self::DEFAULT
    }
}

fn key_code(name: &str) -> Option<u32> {
    if let Some((_, code)) = NAMED
        .iter()
        .find(|(named, _)| named.eq_ignore_ascii_case(name))
    {
        return Some(*code);
    }
    if let Some(hex) = name.strip_prefix("0x").or_else(|| name.strip_prefix("0X")) {
        return u32::from_str_radix(hex, 16)
            .ok()
            .filter(|code| (1..=0xFE).contains(code));
    }
    let mut chars = name.chars();
    if let (Some(char), None) = (chars.next(), chars.clone().next()) {
        let char = char.to_ascii_uppercase();
        if char.is_ascii_uppercase() || char.is_ascii_digit() {
            return Some(u32::from(char));
        }
    }
    let number: u32 = name
        .strip_prefix(['F', 'f'])?
        .parse()
        .ok()
        .filter(|number| (1..=24).contains(number))?;
    Some(0x70 + number - 1)
}

fn key_name(code: u32) -> String {
    if let Some((name, _)) = NAMED.iter().find(|(_, named)| *named == code) {
        return (*name).to_owned();
    }
    match code {
        0x30..=0x39 | 0x41..=0x5A => char::from_u32(code).map_or_else(String::new, String::from),
        0x70..=0x87 => format!("F{}", code - 0x70 + 1),
        _ => format!("0x{code:02X}"),
    }
}

impl fmt::Display for Hotkey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let Modifiers {
            control,
            alt,
            shift,
            win,
        } = self.modifiers;
        for (on, name) in [
            (control, "Ctrl"),
            (alt, "Alt"),
            (shift, "Shift"),
            (win, "Win"),
        ] {
            if on {
                write!(f, "{name}+")?;
            }
        }
        write!(f, "{}", key_name(self.key))
    }
}
