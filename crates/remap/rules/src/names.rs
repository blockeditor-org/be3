use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub enum Source {
    Key(u16),
    Button(u16),
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub enum Modifier {
    Shift,
    Ctrl,
    Alt,
    Super,
}

pub const LEFT_CTRL: u16 = 29;
pub const LEFT_SHIFT: u16 = 42;
pub const RIGHT_SHIFT: u16 = 54;
pub const LEFT_ALT: u16 = 56;
pub const RIGHT_CTRL: u16 = 97;
pub const RIGHT_ALT: u16 = 100;
pub const LEFT_SUPER: u16 = 125;
pub const RIGHT_SUPER: u16 = 126;
pub const BUTTON_FIRST: u16 = 0x100;

impl Modifier {
    pub const ALL: [Self; 4] = [Self::Shift, Self::Ctrl, Self::Alt, Self::Super];

    pub const fn keys(self) -> [Source; 2] {
        match self {
            Self::Shift => [Source::Key(LEFT_SHIFT), Source::Key(RIGHT_SHIFT)],
            Self::Ctrl => [Source::Key(LEFT_CTRL), Source::Key(RIGHT_CTRL)],
            Self::Alt => [Source::Key(LEFT_ALT), Source::Key(RIGHT_ALT)],
            Self::Super => [Source::Key(LEFT_SUPER), Source::Key(RIGHT_SUPER)],
        }
    }

    pub const fn name(self) -> &'static str {
        match self {
            Self::Shift => "Shift",
            Self::Ctrl => "Ctrl",
            Self::Alt => "Alt",
            Self::Super => "Super",
        }
    }

    pub fn named(name: &str) -> Option<Self> {
        Some(match name.to_ascii_lowercase().as_str() {
            "shift" => Self::Shift,
            "ctrl" | "control" => Self::Ctrl,
            "alt" => Self::Alt,
            "super" | "win" | "meta" | "logo" => Self::Super,
            _ => return None,
        })
    }

    pub fn of(source: Source) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|modifier| modifier.keys().contains(&source))
    }
}

const KEYS: &[(&str, u16)] = &[
    ("Escape", 1),
    ("1", 2),
    ("2", 3),
    ("3", 4),
    ("4", 5),
    ("5", 6),
    ("6", 7),
    ("7", 8),
    ("8", 9),
    ("9", 10),
    ("0", 11),
    ("Minus", 12),
    ("Equal", 13),
    ("Backspace", 14),
    ("Tab", 15),
    ("Q", 16),
    ("W", 17),
    ("E", 18),
    ("R", 19),
    ("T", 20),
    ("Y", 21),
    ("U", 22),
    ("I", 23),
    ("O", 24),
    ("P", 25),
    ("LeftBracket", 26),
    ("RightBracket", 27),
    ("Enter", 28),
    ("LeftCtrl", LEFT_CTRL),
    ("A", 30),
    ("S", 31),
    ("D", 32),
    ("F", 33),
    ("G", 34),
    ("H", 35),
    ("J", 36),
    ("K", 37),
    ("L", 38),
    ("Semicolon", 39),
    ("Apostrophe", 40),
    ("Grave", 41),
    ("LeftShift", LEFT_SHIFT),
    ("Backslash", 43),
    ("Z", 44),
    ("X", 45),
    ("C", 46),
    ("V", 47),
    ("B", 48),
    ("N", 49),
    ("M", 50),
    ("Comma", 51),
    ("Period", 52),
    ("Slash", 53),
    ("RightShift", RIGHT_SHIFT),
    ("NumpadMultiply", 55),
    ("LeftAlt", LEFT_ALT),
    ("Space", 57),
    ("CapsLock", 58),
    ("F1", 59),
    ("F2", 60),
    ("F3", 61),
    ("F4", 62),
    ("F5", 63),
    ("F6", 64),
    ("F7", 65),
    ("F8", 66),
    ("F9", 67),
    ("F10", 68),
    ("NumLock", 69),
    ("ScrollLock", 70),
    ("Numpad7", 71),
    ("Numpad8", 72),
    ("Numpad9", 73),
    ("NumpadMinus", 74),
    ("Numpad4", 75),
    ("Numpad5", 76),
    ("Numpad6", 77),
    ("NumpadPlus", 78),
    ("Numpad1", 79),
    ("Numpad2", 80),
    ("Numpad3", 81),
    ("Numpad0", 82),
    ("NumpadDot", 83),
    ("F11", 87),
    ("F12", 88),
    ("NumpadEnter", 96),
    ("RightCtrl", RIGHT_CTRL),
    ("NumpadDivide", 98),
    ("PrintScreen", 99),
    ("RightAlt", RIGHT_ALT),
    ("Home", 102),
    ("Up", 103),
    ("PageUp", 104),
    ("Left", 105),
    ("Right", 106),
    ("End", 107),
    ("Down", 108),
    ("PageDown", 109),
    ("Insert", 110),
    ("Delete", 111),
    ("Mute", 113),
    ("VolumeDown", 114),
    ("VolumeUp", 115),
    ("Pause", 119),
    ("LeftSuper", LEFT_SUPER),
    ("RightSuper", RIGHT_SUPER),
    ("Menu", 127),
    ("NextTrack", 163),
    ("PlayPause", 164),
    ("PreviousTrack", 165),
    ("StopMedia", 166),
    ("F13", 183),
    ("F14", 184),
    ("F15", 185),
    ("F16", 186),
    ("F17", 187),
    ("F18", 188),
    ("F19", 189),
    ("F20", 190),
    ("F21", 191),
    ("F22", 192),
    ("F23", 193),
    ("F24", 194),
    ("BrightnessDown", 224),
    ("BrightnessUp", 225),
    ("MicMute", 248),
];

const BUTTONS: &[(&str, u16)] = &[
    ("MouseLeft", 0x110),
    ("MouseRight", 0x111),
    ("MouseMiddle", 0x112),
    ("MouseSide", 0x113),
    ("MouseExtra", 0x114),
    ("MouseForward", 0x115),
    ("MouseBack", 0x116),
    ("MouseTask", 0x117),
];

const ALIASES: &[(&str, &str)] = &[
    ("Esc", "Escape"),
    ("Return", "Enter"),
    ("Shift", "LeftShift"),
    ("LShift", "LeftShift"),
    ("RShift", "RightShift"),
    ("Ctrl", "LeftCtrl"),
    ("Control", "LeftCtrl"),
    ("LCtrl", "LeftCtrl"),
    ("RCtrl", "RightCtrl"),
    ("Alt", "LeftAlt"),
    ("LAlt", "LeftAlt"),
    ("RAlt", "RightAlt"),
    ("AltGr", "RightAlt"),
    ("Super", "LeftSuper"),
    ("Win", "LeftSuper"),
    ("LWin", "LeftSuper"),
    ("RWin", "RightSuper"),
    ("Meta", "LeftSuper"),
    ("Del", "Delete"),
    ("Ins", "Insert"),
    ("PgUp", "PageUp"),
    ("PgDn", "PageDown"),
    ("Dot", "Period"),
    ("Backtick", "Grave"),
    ("Quote", "Apostrophe"),
    ("PrtSc", "PrintScreen"),
    ("Break", "Pause"),
    ("AppsKey", "Menu"),
    ("Volume_Up", "VolumeUp"),
    ("Volume_Down", "VolumeDown"),
    ("Volume_Mute", "Mute"),
    ("Media_Play_Pause", "PlayPause"),
    ("Media_Next", "NextTrack"),
    ("Media_Prev", "PreviousTrack"),
    ("Media_Stop", "StopMedia"),
    ("LButton", "MouseLeft"),
    ("RButton", "MouseRight"),
    ("MButton", "MouseMiddle"),
    ("XButton1", "MouseSide"),
    ("XButton2", "MouseExtra"),
];

impl Source {
    pub fn named(name: &str) -> Option<Self> {
        let name = name.trim();
        if let Some(code) = raw(name, "key:") {
            return (code < BUTTON_FIRST).then_some(Self::Key(code));
        }
        if let Some(code) = raw(name, "button:") {
            return (code >= BUTTON_FIRST).then_some(Self::Button(code));
        }
        let name = ALIASES
            .iter()
            .find(|(alias, _)| alias.eq_ignore_ascii_case(name))
            .map_or(name, |(_, canonical)| canonical);
        KEYS.iter()
            .find(|(known, _)| known.eq_ignore_ascii_case(name))
            .map(|(_, code)| Self::Key(*code))
            .or_else(|| {
                BUTTONS
                    .iter()
                    .find(|(known, _)| known.eq_ignore_ascii_case(name))
                    .map(|(_, code)| Self::Button(*code))
            })
    }

    pub fn name(self) -> String {
        let (table, code, raw) = match self {
            Self::Key(code) => (KEYS, code, "Key"),
            Self::Button(code) => (BUTTONS, code, "Button"),
        };
        table.iter().find(|(_, known)| *known == code).map_or_else(
            || format!("{raw}:{code:#x}"),
            |(name, _)| (*name).to_owned(),
        )
    }

    pub const fn code(self) -> u16 {
        match self {
            Self::Key(code) | Self::Button(code) => code,
        }
    }

    pub const fn from_code(code: u16) -> Self {
        match code >= BUTTON_FIRST {
            true => Self::Button(code),
            false => Self::Key(code),
        }
    }
}

fn raw(name: &str, prefix: &str) -> Option<u16> {
    let head = name.get(..prefix.len())?;
    if !head.eq_ignore_ascii_case(prefix) {
        return None;
    }
    let code = name[prefix.len()..].trim();
    match code.strip_prefix("0x").or_else(|| code.strip_prefix("0X")) {
        Some(hex) => u16::from_str_radix(hex, 16).ok(),
        None => code.parse().ok(),
    }
}
