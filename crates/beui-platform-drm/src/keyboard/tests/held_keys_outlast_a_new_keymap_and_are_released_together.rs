use super::*;

#[test]
fn held_keys_outlast_a_new_keymap_and_are_released_together() {
    let mut old = Keyboard::new(&InputConfig::default()).expect("the default keymap compiles");
    old.key(CONTROL, true);
    let mut keyboard = Keyboard::new(&InputConfig::default()).expect("the default keymap compiles");
    assert_eq!(keyboard.carry(&old), None, "Ctrl was held and still is");
    assert_eq!(keyboard.modifiers(), Modifiers::CTRL);

    let released = keyboard.release_all();
    assert!(released.contains(&Input::Modifiers(Modifiers::NONE)));
    assert!(released.iter().any(|input| matches!(
        input,
        Input::Key(Keystroke {
            pressed: false,
            evdev: Some(CONTROL),
            ..
        })
    )));
    assert_eq!(keyboard.key(CONTROL, false), Pressed::default());
}
