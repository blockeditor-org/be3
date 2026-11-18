use super::*;

#[test]
fn a_key_switching_terminals_is_not_released_into_the_session() {
    let mut keyboard = Keyboard::new(&InputConfig::default()).expect("the default keymap compiles");
    keyboard.key(CONTROL, true);
    keyboard.key(ALT, true);
    let switched = keyboard.key(KEY_F2, true);
    assert_eq!(switched.terminal, Some(2));
    assert!(
        !switched
            .inputs
            .iter()
            .any(|input| matches!(input, Input::Key(_))),
        "the session keeps the press for itself"
    );

    let released = keyboard.release_all();
    let keys: Vec<u32> = released
        .iter()
        .filter_map(|input| match input {
            Input::Key(Keystroke { evdev, .. }) => *evdev,
            _ => None,
        })
        .collect();
    assert_eq!(
        keys,
        [CONTROL, ALT],
        "on return Ctrl and Alt are let go, and F2, whose press went nowhere, is not released"
    );
    assert_eq!(keyboard.modifiers(), Modifiers::NONE);
}
