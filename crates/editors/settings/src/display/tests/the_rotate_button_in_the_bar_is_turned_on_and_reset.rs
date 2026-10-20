use super::*;

#[test]
fn the_rotate_button_in_the_bar_is_turned_on_and_reset() {
    let settings = Harness::new();
    assert!(
        !settings.editor.shown(ROTATE_IN_BAR),
        "only a session has a bar to put it in"
    );

    let mut settings = Harness::in_a_session(vec![gaming_monitor()]);
    assert_eq!(settings.content().root().rotate_in_bar, None);
    assert!(!settings.editor.shown(ROTATE_IN_BAR_RESET));

    settings.editor.click(ROTATE_IN_BAR);
    settings.run();
    assert_eq!(settings.content().root().rotate_in_bar, Some(true));
    settings
        .editor
        .snapshot("the_rotate_button_in_the_bar_is_turned_on");

    settings.editor.click(ROTATE_IN_BAR_RESET);
    settings.run();
    assert_eq!(settings.content().root().rotate_in_bar, None);
}
