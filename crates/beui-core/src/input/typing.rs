use crate::input::{Input, Key, Keystroke, Modifiers};

pub fn set_modifier(modifiers: &mut Modifiers, key: Key, pressed: bool) {
    match key {
        Key::Shift => modifiers.shift = pressed,
        Key::Ctrl => modifiers.ctrl = pressed,
        Key::Alt => modifiers.alt = pressed,
        Key::Logo => modifiers.logo = pressed,
        _ => {}
    }
}

pub fn modifier_event(key: Key, pressed: bool, modifiers: Modifiers, events: &mut Vec<Input>) {
    events.push(stroke(key, pressed, false, None));
    events.push(Input::Modifiers(modifiers));
}

pub fn chord(chord: &str, held: Modifiers, events: &mut Vec<Input>) -> Result<(), String> {
    let mut parts: Vec<&str> = chord.split('+').collect();
    if chord.ends_with("++") || chord == "+" {
        parts.retain(|part| !part.is_empty());
        parts.push("+");
    }
    let (name, modifiers) = parts
        .split_last()
        .filter(|(name, _)| !name.is_empty())
        .ok_or_else(|| format!("{chord:?} names no key"))?;
    let key = Key::from_name(name).ok_or_else(|| format!("{name:?} is not a key beui knows"))?;
    let mut wanted = Modifiers::NONE;
    for name in modifiers {
        let modifier = Key::modifier_named(name)
            .ok_or_else(|| format!("{name:?} in {chord:?} is not shift, ctrl, alt or super"))?;
        set_modifier(&mut wanted, modifier, true);
    }
    press_chord(key, wanted, held, events);
    Ok(())
}

pub fn press_chord(key: Key, wanted: Modifiers, held: Modifiers, events: &mut Vec<Input>) {
    let mut pressed = Vec::new();
    let mut state = held;
    for (modifier, on) in [
        (Key::Ctrl, wanted.ctrl),
        (Key::Alt, wanted.alt),
        (Key::Shift, wanted.shift),
        (Key::Logo, wanted.logo),
    ] {
        if on && !held_by(held, modifier) {
            set_modifier(&mut state, modifier, true);
            modifier_event(modifier, true, state, events);
            pressed.push(modifier);
        }
    }
    let typed = text(key, state.shift).filter(|_| !(state.ctrl || state.alt || state.logo));
    events.push(stroke(key, true, false, typed));
    events.push(stroke(key, false, false, None));
    for modifier in pressed.into_iter().rev() {
        set_modifier(&mut state, modifier, false);
        modifier_event(modifier, false, state, events);
    }
}

pub fn keyable(character: char) -> Option<(Key, bool)> {
    typeable(character)
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
    events: &mut Vec<Input>,
) {
    let typed = text(key, modifiers.shift)
        .filter(|_| pressed && !(modifiers.ctrl || modifiers.alt || modifiers.logo));
    events.push(stroke(key, pressed, repeat, typed));
}

pub fn typed(text: &str, held: Modifiers, events: &mut Vec<Input>) {
    let mut loose = String::new();
    for character in text.chars() {
        let key = match character {
            '\n' => Some((Key::Enter, false, None)),
            '\t' => Some((Key::Tab, false, None)),
            character => typeable(character).map(|(key, shift)| (key, shift, Some(character))),
        };
        let Some((key, shift, character)) = key else {
            loose.push(character);
            continue;
        };
        if !loose.is_empty() {
            events.push(Input::Text(std::mem::take(&mut loose)));
        }
        let changed = shift != held.shift;
        let mut state = held;
        if changed {
            state.shift = shift;
            modifier_event(Key::Shift, shift, state, events);
        }
        let text = character
            .map(String::from)
            .filter(|_| !(state.ctrl || state.alt || state.logo));
        events.push(stroke(key, true, false, text));
        events.push(stroke(key, false, false, None));
        if changed {
            modifier_event(Key::Shift, held.shift, held, events);
        }
    }
    if !loose.is_empty() {
        events.push(Input::Text(loose));
    }
}

fn typeable(wanted: char) -> Option<(Key, bool)> {
    if wanted.is_ascii_uppercase() {
        return Key::unshifted(wanted.to_ascii_lowercase()).map(|key| (key, true));
    }
    let shifted = match wanted {
        '!' => Key::One,
        '@' => Key::Two,
        '#' => Key::Three,
        '$' => Key::Four,
        '%' => Key::Five,
        '^' => Key::Six,
        '&' => Key::Seven,
        '*' => Key::Eight,
        '(' => Key::Nine,
        ')' => Key::Zero,
        '_' => Key::Minus,
        '+' => Key::Plus,
        ':' => Key::Semicolon,
        '"' => Key::Quote,
        '<' => Key::Comma,
        '>' => Key::Period,
        '?' => Key::Slash,
        '|' => Key::Backslash,
        '~' => Key::Backtick,
        '{' => Key::BracketLeft,
        '}' => Key::BracketRight,
        other => return Key::unshifted(other).map(|key| (key, false)),
    };
    Some((shifted, true))
}

fn stroke(key: Key, pressed: bool, repeat: bool, text: Option<String>) -> Input {
    Input::Key(Keystroke {
        key: Some(key),
        pressed,
        repeat,
        evdev: key.evdev(),
        text,
    })
}

pub fn chord_name(key: Key, modifiers: Modifiers) -> Option<String> {
    let mut chord = String::new();
    for (held, name) in [
        (modifiers.ctrl, "ctrl+"),
        (modifiers.alt, "alt+"),
        (modifiers.shift, "shift+"),
        (modifiers.logo, "super+"),
    ] {
        if held {
            chord.push_str(name);
        }
    }
    chord.push_str(&key.name()?);
    Some(chord)
}

fn text(key: Key, shift: bool) -> Option<String> {
    let character = "abcdefghijklmnopqrstuvwxyz0123456789-=,./\\;'`[] "
        .chars()
        .find(|character| Key::unshifted(*character) == Some(key))?;
    match (shift, character.is_ascii_lowercase()) {
        (true, true) => Some(character.to_ascii_uppercase().to_string()),
        (true, false) => None,
        (false, _) => Some(character.to_string()),
    }
}
