use super::*;

#[test]
fn a_press_whose_release_was_lost_is_let_go_by_the_next_press_and_release() {
    let mut keyboard = Keyboard::new(&InputConfig::default()).expect("the default keymap compiles");
    let held = keyboard.key(CONTROL, true);
    assert!(held.inputs.contains(&Input::Modifiers(Modifiers::CTRL)));

    let again = keyboard.key(CONTROL, true);
    assert_eq!(again, Pressed::default(), "a held key is not pressed again");
    let released = keyboard.key(CONTROL, false);
    assert!(released.inputs.contains(&Input::Modifiers(Modifiers::NONE)));
    assert_eq!(keyboard.modifiers(), Modifiers::NONE);

    assert_eq!(
        keyboard.key(CONTROL, false),
        Pressed::default(),
        "a key not held is not released"
    );
}
