use beui::{Key, KeyPress};
use terminal_types::TerminalKey;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TerminalInput {
    Key(TerminalKey),
    Paste(String),
}

pub fn key_event(press: KeyPress) -> Option<TerminalKey> {
    let modifiers = press.modifiers;
    if press.key.is_modifier() || press.key.is_media() || press.key == Key::BrowserBack {
        return None;
    }
    if modifiers.ctrl && modifiers.shift && matches!(press.key, Key::C | Key::V) {
        return None;
    }
    if !modifiers.ctrl && !modifiers.alt && !modifiers.logo && types_text(press.key) {
        return None;
    }
    Some(TerminalKey {
        key: press.key,
        modifiers,
        text: String::new(),
        pressed: press.pressed,
    })
}

pub fn text_event(text: &str) -> TerminalKey {
    TerminalKey {
        key: Key::Unidentified,
        modifiers: beui::Modifiers::NONE,
        text: text.to_owned(),
        pressed: true,
    }
}

fn types_text(key: Key) -> bool {
    matches!(
        key,
        Key::A
            | Key::B
            | Key::C
            | Key::D
            | Key::E
            | Key::F
            | Key::G
            | Key::H
            | Key::I
            | Key::J
            | Key::K
            | Key::L
            | Key::M
            | Key::N
            | Key::O
            | Key::P
            | Key::Q
            | Key::R
            | Key::S
            | Key::T
            | Key::U
            | Key::V
            | Key::W
            | Key::X
            | Key::Y
            | Key::Z
            | Key::Zero
            | Key::One
            | Key::Two
            | Key::Three
            | Key::Four
            | Key::Five
            | Key::Six
            | Key::Seven
            | Key::Eight
            | Key::Nine
            | Key::Backtick
            | Key::Minus
            | Key::Plus
            | Key::BracketLeft
            | Key::BracketRight
            | Key::Backslash
            | Key::Semicolon
            | Key::Quote
            | Key::Comma
            | Key::Period
            | Key::Slash
            | Key::Space
            | Key::Unidentified
    )
}
