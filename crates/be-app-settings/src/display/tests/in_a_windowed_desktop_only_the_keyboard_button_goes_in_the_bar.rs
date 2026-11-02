use super::*;

#[test]
fn in_a_windowed_desktop_only_the_keyboard_button_goes_in_the_bar() {
    let mut settings = Harness::in_a_window();
    assert!(settings.editor.shown(KEYBOARD_IN_BAR));
    assert!(
        !settings.editor.shown(ROTATE_IN_BAR),
        "a window has no screen of its own to turn"
    );

    settings.editor.click(KEYBOARD_IN_BAR);
    settings.run();
    assert_eq!(settings.content().root().keyboard_in_bar, Some(true));
    settings
        .editor
        .snapshot("in_a_windowed_desktop_only_the_keyboard_button_goes_in_the_bar");
}
