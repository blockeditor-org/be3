use super::*;

#[test]
fn the_keyboard_button_in_the_bar_is_turned_on_and_reset() {
    let settings = Harness::new();
    assert!(
        !settings.editor.shown(KEYBOARD_IN_BAR),
        "only a session has a bar to put it in"
    );

    let mut settings = Harness::in_a_session(vec![gaming_monitor()]);
    assert_eq!(settings.content().root().keyboard_in_bar, None);
    assert!(!settings.content().root().keyboard_in_bar());

    settings.editor.click(KEYBOARD_IN_BAR);
    settings.run();
    assert_eq!(settings.content().root().keyboard_in_bar, Some(true));
    assert_eq!(
        settings.content().root().rotate_in_bar,
        None,
        "the rotate button is a setting of its own"
    );

    settings.reset(KEYBOARD_IN_BAR);
    assert_eq!(settings.content().root().keyboard_in_bar, None);
}
