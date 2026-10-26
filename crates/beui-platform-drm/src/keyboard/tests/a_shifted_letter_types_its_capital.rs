use super::*;

use beui::Key;

#[test]
fn a_shifted_letter_types_its_capital() {
    let mut keyboard = Keyboard::new(&InputConfig::default()).expect("the default keymap compiles");
    keyboard.key(SHIFT, true);
    let pressed = keyboard.key(KEY_A, true);

    let typed = Keystroke {
        key: Some(Key::A),
        pressed: true,
        repeat: false,
        evdev: Some(KEY_A),
        text: Some("A".to_owned()),
    };
    assert!(pressed.inputs.contains(&Input::Key(typed.clone())));
    assert_eq!(
        pressed.repeat,
        Some(Keystroke {
            repeat: true,
            ..typed
        }),
        "holding a letter repeats it"
    );
}
