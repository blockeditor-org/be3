use super::*;

#[test]
fn ui_settings_keep_the_zoom_in_bounds() {
    let mut settings = UiSettingsContent::default();
    assert_eq!(settings.zoom(), 1.0);

    settings.apply(&UiSettingsOp::SetZoom { zoom: 1.5 });
    assert_eq!(settings.zoom(), 1.5);
    settings.apply(&UiSettingsOp::SetZoom { zoom: f32::NAN });
    assert_eq!(settings.zoom(), 1.5);
    settings.apply(&UiSettingsOp::SetZoom { zoom: 40.0 });
    assert_eq!(settings.zoom(), 3.0);

    assert_eq!(UiSettingsContent::decode(&settings.encode()), Ok(settings));
    assert_eq!(
        UiSettingsContent::decode(&0.01_f32.to_le_bytes()).map(|settings| settings.zoom()),
        Ok(0.5)
    );
    assert_eq!(
        UiSettingsContent::decode(&f32::INFINITY.to_le_bytes()),
        Err(ContentError::Malformed("a zoom is a finite number"))
    );
}
