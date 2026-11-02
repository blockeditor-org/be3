use super::*;

#[test]
fn the_rotate_button_shows_when_the_display_settings_ask_for_it_and_a_screen_is_reported() {
    let mut fixture = Fixture::new();
    fixture.report_screens(vec![epaper_panel()]);
    assert!(
        !fixture.test.shown("desktop.rotate"),
        "the display settings have not asked for it"
    );

    fixture.with_display_settings(DisplaySettingsContent::default(), None);
    assert!(!fixture.test.shown("desktop.rotate"));

    let mut fixture = Fixture::new();
    fixture.with_display_settings(rotate_in_bar(), None);
    assert!(
        !fixture.test.shown("desktop.rotate"),
        "a desktop that reports no screen has none to turn"
    );

    fixture.report_screens(vec![epaper_panel()]);
    assert!(fixture.test.shown("desktop.rotate"));
    fixture.test.snapshot("the_rotate_button_in_the_bar");

    fixture.report_screens(Vec::new());
    assert!(!fixture.test.shown("desktop.rotate"));
}
