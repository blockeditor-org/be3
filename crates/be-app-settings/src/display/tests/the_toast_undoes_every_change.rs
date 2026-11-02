use super::*;

#[test]
fn the_toast_undoes_every_change() {
    let mut start = DisplaySettingsContent::default();
    start.apply(&DisplaySettings::set_theme(Some(ThemeKind::EPaper)));
    let mut settings = Harness::new();
    settings.editor.hold(None, start);
    let mut settings = settings.on_this_device();
    settings.choose(THEME, 1);
    settings.choose(SCALE, 6);
    assert!(settings.says("Theme and scale now apply on your devices"));

    settings.editor.click("toast.0.action.undo");
    settings.run();
    assert_eq!(settings.every_device().theme, Some(ThemeKind::EPaper));
    assert_eq!(settings.every_device().scale_percent, None);
    assert_eq!(settings.here(), DisplaySettings::default());
    assert!(!settings.says("now apply"));
}
