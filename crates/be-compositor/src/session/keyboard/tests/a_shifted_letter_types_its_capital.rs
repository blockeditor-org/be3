use super::*;

use beui::Key;

#[test]
fn a_shifted_letter_types_its_capital() {
    let mut keyboard = Keyboard::new().expect("the default keymap compiles");
    keyboard.key(SHIFT, true);
    let pressed = keyboard.key(KEY_A, true);

    assert!(pressed.events.contains(&Event::PhysicalKey {
        code: KEY_A,
        pressed: true,
    }));
    assert!(pressed.events.contains(&Event::Key {
        key: Key::A,
        pressed: true,
        repeat: false,
        modifiers: Modifiers::SHIFT,
    }));
    assert!(pressed.events.contains(&Event::Text("A".to_owned())));
    assert!(pressed.repeat.is_some(), "holding a letter repeats it");

    let released = keyboard.key(KEY_A, false);
    assert!(
        !released
            .events
            .iter()
            .any(|event| matches!(event, Event::Text(_)))
    );
}
