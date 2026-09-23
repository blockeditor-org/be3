use super::*;

#[test]
fn ui_settings_keep_the_zoom_in_bounds() {
    let settings = UiSettingsContent::default();
    assert_eq!(settings.root().zoom(), 1.0);

    let settings = edited(&settings, [UiSettings::set_zoom(1.5)]);
    assert_eq!(settings.root().zoom(), 1.5);
    let settings = edited(&settings, [UiSettings::set_zoom(f32::NAN)]);
    assert_eq!(settings.root().zoom(), 1.0);
    let settings = edited(&settings, [UiSettings::set_zoom(40.0)]);
    assert_eq!(settings.root().zoom(), 3.0);
}
