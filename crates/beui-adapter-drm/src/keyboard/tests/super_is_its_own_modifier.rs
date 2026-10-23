use super::*;

#[test]
fn super_is_its_own_modifier() {
    let mut keyboard = Keyboard::new(&InputConfig::default()).expect("the default keymap compiles");
    let held = keyboard.key(LEFT_SUPER, true);
    assert!(held.inputs.contains(&Input::Modifiers(Modifiers::LOGO)));
    assert_eq!(held.repeat, None, "holding a modifier does not repeat it");

    keyboard.key(KEY_F, true);
    keyboard.key(KEY_F, false);
    let released = keyboard.key(LEFT_SUPER, false);
    assert!(released.inputs.contains(&Input::Modifiers(Modifiers::NONE)));
}
