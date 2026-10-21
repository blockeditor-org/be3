use super::*;

#[test]
fn keeping_a_setting_copies_it_here_and_later_changes_stay_here() {
    let mut settings = Harness::new().on_this_device();
    settings.choose(THEME, 1);

    settings.open_menu(THEME);
    settings.editor.snapshot("a_settings_rows_menu");
    settings.editor.click("display.theme.keep");
    settings.run();
    assert_eq!(
        settings.here().theme,
        Some(ThemeKind::Light),
        "keeping copies the theme in use"
    );
    assert!(settings.says("Kept on this device"));
    settings.editor.snapshot("a_setting_kept_on_this_device");

    settings.choose(THEME, 3);
    assert_eq!(settings.here().theme, Some(ThemeKind::OledDark));
    assert_eq!(
        settings.every_device().theme,
        Some(ThemeKind::Light),
        "the other devices keep theirs"
    );

    settings.keep(THEME);
    assert_eq!(settings.here().theme, None, "letting go deletes it here");
    assert!(!settings.says("Kept on this device"));
    assert!(settings.says("Light"));

    settings.keep(SCALE);
    assert_eq!(
        settings.here().scale_percent,
        Some(DisplaySettings::DEFAULT_SCALE),
        "keeping an unset setting keeps its default"
    );
}
