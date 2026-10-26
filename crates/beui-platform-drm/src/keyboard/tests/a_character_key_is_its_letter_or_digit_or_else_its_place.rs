use super::*;

use beui::Key;

fn named(keyboard: &mut Keyboard, code: u32) -> Option<Key> {
    let pressed = keyboard.key(code, true);
    keyboard.key(code, false);
    pressed.inputs.into_iter().find_map(|input| match input {
        Input::Key(Keystroke { key, .. }) => key,
        _ => None,
    })
}

#[test]
fn a_character_key_is_its_letter_or_digit_or_else_its_place() {
    let french = InputConfig {
        layout: "fr".to_owned(),
        ..InputConfig::default()
    };
    let mut keyboard = Keyboard::new(&french).expect("the French keymap compiles");
    for (code, key) in [(KEY_1, Key::One), (KEY_Q, Key::A), (KEY_M, Key::M)] {
        assert_eq!(named(&mut keyboard, code), Some(key), "evdev {code}");
        keyboard.key(SHIFT, true);
        assert_eq!(
            named(&mut keyboard, code),
            Some(key),
            "shifted evdev {code}"
        );
        keyboard.key(SHIFT, false);
    }
}
