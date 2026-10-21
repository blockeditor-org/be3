use super::*;

#[test]
fn resetting_a_kept_setting_clears_it_here() {
    let mut kept = DisplaySettingsContent::default();
    kept.apply(&DisplaySettings::set_theme(Some(ThemeKind::EPaper)));
    let mut settings = Harness::new().on_this_device_with(Some(kept));
    settings.choose(THEME, 0);
    assert_eq!(
        settings.here().theme,
        None,
        "choosing the default clears it here"
    );

    settings.choose(THEME, 1);
    assert_eq!(settings.every_device().theme, Some(ThemeKind::Light));
    settings.keep(THEME);
    settings.choose(THEME, 4);
    assert_eq!(settings.here().theme, Some(ThemeKind::EPaper));

    settings.reset(THEME);
    assert_eq!(settings.here().theme, None);
    assert_eq!(
        settings.every_device().theme,
        Some(ThemeKind::Light),
        "a reset clears only the layer it would write to"
    );

    settings.reset(THEME);
    assert_eq!(settings.every_device().theme, None);
}
