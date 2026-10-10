use crate::input::{Event, Key, Modifiers};

pub fn modifier(name: &str) -> Option<Key> {
    match name.to_ascii_lowercase().as_str() {
        "shift" => Some(Key::Shift),
        "ctrl" | "control" => Some(Key::Ctrl),
        "alt" => Some(Key::Alt),
        "super" | "logo" | "meta" | "win" => Some(Key::Logo),
        _ => None,
    }
}

pub fn set_modifier(modifiers: &mut Modifiers, key: Key, pressed: bool) {
    match key {
        Key::Shift => modifiers.shift = pressed,
        Key::Ctrl => modifiers.ctrl = pressed,
        Key::Alt => modifiers.alt = pressed,
        Key::Logo => modifiers.logo = pressed,
        _ => {}
    }
}

pub fn modifier_event(key: Key, pressed: bool, modifiers: Modifiers, events: &mut Vec<Event>) {
    physical(key, pressed, events);
    events.push(Event::Key {
        key,
        pressed,
        repeat: false,
        modifiers,
    });
    events.push(Event::Modifiers(modifiers));
}

pub fn chord(chord: &str, held: Modifiers, events: &mut Vec<Event>) -> Result<(), String> {
    let mut parts: Vec<&str> = chord.split('+').collect();
    if chord.ends_with("++") || chord == "+" {
        parts.retain(|part| !part.is_empty());
        parts.push("+");
    }
    let (name, modifiers) = parts
        .split_last()
        .filter(|(name, _)| !name.is_empty())
        .ok_or_else(|| format!("{chord:?} names no key"))?;
    let key = named(name).ok_or_else(|| format!("{name:?} is not a key beui knows"))?;
    let mut pressed = Vec::new();
    let mut state = held;
    for name in modifiers {
        let modifier = modifier(name)
            .ok_or_else(|| format!("{name:?} in {chord:?} is not shift, ctrl, alt or super"))?;
        set_modifier(&mut state, modifier, true);
        modifier_event(modifier, true, state, events);
        pressed.push(modifier);
    }
    press(key, state, events);
    if !(state.ctrl || state.alt || state.logo)
        && let Some(text) = text(key, state.shift)
    {
        events.push(Event::Text(text.to_owned()));
    }
    for modifier in pressed.into_iter().rev() {
        set_modifier(&mut state, modifier, false);
        set_modifier(&mut state, modifier, held_by(held, modifier));
        modifier_event(modifier, false, state, events);
    }
    Ok(())
}

fn held_by(held: Modifiers, key: Key) -> bool {
    match key {
        Key::Shift => held.shift,
        Key::Ctrl => held.ctrl,
        Key::Alt => held.alt,
        Key::Logo => held.logo,
        _ => false,
    }
}

pub fn key_event(
    key: Key,
    pressed: bool,
    repeat: bool,
    modifiers: Modifiers,
    events: &mut Vec<Event>,
) {
    if !repeat {
        physical(key, pressed, events);
    }
    events.push(Event::Key {
        key,
        pressed,
        repeat,
        modifiers,
    });
    if pressed
        && !(modifiers.ctrl || modifiers.alt || modifiers.logo)
        && let Some(text) = text(key, modifiers.shift)
    {
        events.push(Event::Text(text));
    }
}

pub fn press(key: Key, modifiers: Modifiers, events: &mut Vec<Event>) {
    for pressed in [true, false] {
        physical(key, pressed, events);
        events.push(Event::Key {
            key,
            pressed,
            repeat: false,
            modifiers,
        });
    }
}

fn physical(key: Key, pressed: bool, events: &mut Vec<Event>) {
    if cfg!(target_os = "linux")
        && let Some(code) = evdev(key)
    {
        events.push(Event::PhysicalKey { code, pressed });
    }
}

pub fn named(name: &str) -> Option<Key> {
    if let Some(key) = modifier(name) {
        return Some(key);
    }
    let lower = name.to_ascii_lowercase();
    let mut chars = name.chars();
    if let (Some(single), None) = (chars.next(), chars.next())
        && let Some(key) = character(single.to_ascii_lowercase())
    {
        return Some(key);
    }
    if let Some(number) = lower.strip_prefix('f')
        && let Ok(number) = number.parse::<usize>()
    {
        return FUNCTION.get(number.wrapping_sub(1)).copied();
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

fn character(character: char) -> Option<Key> {
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

fn text(key: Key, shift: bool) -> Option<String> {
    if let Some(index) = LETTERS.iter().position(|letter| *letter == key) {
        let letter = (b'a' + index as u8) as char;
        return Some(match shift {
            true => letter.to_ascii_uppercase().to_string(),
            false => letter.to_string(),
        });
    }
    if shift {
        return None;
    }
    if let Some(index) = DIGITS.iter().position(|digit| *digit == key) {
        return Some(index.to_string());
    }
    let character = match key {
        Key::Minus => '-',
        Key::Plus => '=',
        Key::Comma => ',',
        Key::Period => '.',
        Key::Slash => '/',
        Key::Backslash => '\\',
        Key::Semicolon => ';',
        Key::Quote => '\'',
        Key::Backtick => '`',
        Key::BracketLeft => '[',
        Key::BracketRight => ']',
        Key::Space => ' ',
        _ => return None,
    };
    Some(character.to_string())
}

fn evdev(key: Key) -> Option<u32> {
    const LETTER_CODES: [u32; 26] = [
        30, 48, 46, 32, 18, 33, 34, 35, 23, 36, 37, 38, 50, 49, 24, 25, 16, 19, 31, 20, 22, 47, 17,
        45, 21, 44,
    ];
    if let Some(index) = LETTERS.iter().position(|letter| *letter == key) {
        return Some(LETTER_CODES[index]);
    }
    if let Some(index) = DIGITS.iter().position(|digit| *digit == key) {
        return Some(if index == 0 { 11 } else { index as u32 + 1 });
    }
    if let Some(index) = FUNCTION.iter().position(|function| *function == key) {
        return Some(match index {
            0..=9 => 59 + index as u32,
            10 => 87,
            11 => 88,
            _ => 183 + (index as u32 - 12),
        });
    }
    Some(match key {
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
