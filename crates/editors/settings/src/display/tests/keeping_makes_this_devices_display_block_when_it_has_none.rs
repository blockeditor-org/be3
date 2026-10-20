use super::*;

#[test]
fn keeping_makes_this_devices_display_block_when_it_has_none() {
    let mut settings = Harness::new().on_this_device_with(None);
    settings.choose(THEME, 2);

    settings.keep(THEME);
    let made = settings
        .editor
        .content::<SettingsProfileContent>(Some(settings.profile))
        .root()
        .kind(DisplaySettings::CONTENT_TYPE);
    assert!(made.is_some(), "the profile gets a display block");
    assert_eq!(settings.here().theme, Some(Theme::Dark));

    settings.keep(SCALE);
    assert_eq!(
        settings
            .editor
            .content::<SettingsProfileContent>(Some(settings.profile))
            .root()
            .kind(DisplaySettings::CONTENT_TYPE),
        made,
        "and only one"
    );
}
