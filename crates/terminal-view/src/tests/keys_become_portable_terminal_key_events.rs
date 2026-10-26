use super::*;
use beui::{Key, KeyPress, Modifiers};

fn press(key: Key, modifiers: Modifiers) -> KeyPress {
    KeyPress {
        key,
        pressed: true,
        repeat: false,
        modifiers,
    }
}

#[test]
fn keys_become_portable_terminal_key_events() {
    let ctrl = Modifiers {
        ctrl: true,
        ..Modifiers::NONE
    };
    let control_c = key_event(press(Key::C, ctrl)).expect("Ctrl+C goes to the shell");
    assert_eq!(control_c.key, Key::C);
    assert!(control_c.modifiers.ctrl);
    assert!(control_c.pressed);
    assert_eq!(
        key_event(press(Key::ArrowUp, Modifiers::NONE)).map(|key| key.key),
        Some(Key::ArrowUp)
    );
    assert_eq!(
        key_event(press(Key::C, Modifiers::NONE)),
        None,
        "a letter arrives as text"
    );
    assert_eq!(key_event(press(Key::Shift, Modifiers::NONE)), None);
    let copy = Modifiers {
        ctrl: true,
        shift: true,
        ..Modifiers::NONE
    };
    assert_eq!(
        key_event(press(Key::V, copy)),
        None,
        "Ctrl+Shift+V is left to paste"
    );

    let typed = text_event("é");
    assert_eq!(typed.key, Key::Unidentified);
    assert_eq!(typed.text, "é");
}
