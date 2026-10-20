use super::*;

#[test]
fn resetting_a_display_returns_it_to_its_default() {
    let mut settings = Harness::in_a_session(vec![gaming_monitor()]);
    settings.choose(REFRESH, 2);
    assert_eq!(
        settings.content().root().mode(MONITOR),
        Some(saved(2560, 1440, 59_951))
    );

    settings.editor.click(RESET);
    settings.run();

    assert_eq!(settings.content().root().mode(MONITOR), None);
    assert!(settings.content().root().screens.is_empty());
}
