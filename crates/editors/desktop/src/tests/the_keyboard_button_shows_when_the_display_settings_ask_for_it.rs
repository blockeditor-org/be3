use super::*;

fn keyboard_in_bar() -> DisplaySettingsContent {
    let mut content = DisplaySettingsContent::default();
    content.apply(&DisplaySettings::set_keyboard_in_bar(Some(true)));
    content
}

#[test]
fn the_keyboard_button_shows_when_the_display_settings_ask_for_it() {
    let mut fixture = Fixture::new();
    fixture.with_display_settings(DisplaySettingsContent::default(), None);
    assert!(
        !fixture.test.shown("desktop.keyboard"),
        "a desktop keeps the keyboard in Quick settings only"
    );

    let mut fixture = Fixture::new();
    fixture.with_display_settings(keyboard_in_bar(), None);
    assert!(fixture.test.shown("desktop.keyboard"));
    let keyboard = fixture.test.rect_of("desktop.keyboard");
    assert!(
        fixture.test.rect_of("desktop.bar.status").contains_rect(keyboard)
            && keyboard.right() < fixture.test.rect_of("desktop.status").left(),
        "it sits on the Status bar before the status button"
    );
}
