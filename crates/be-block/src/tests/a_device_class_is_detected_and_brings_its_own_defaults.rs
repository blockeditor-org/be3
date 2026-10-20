use super::*;
use crate::{DeviceClass, Theme};

#[test]
fn a_device_class_is_detected_and_brings_its_own_defaults() {
    assert_eq!(DeviceClass::detect(true, false), DeviceClass::EpaperTablet);
    assert_eq!(DeviceClass::detect(true, true), DeviceClass::EpaperTablet);
    assert_eq!(DeviceClass::detect(false, true), DeviceClass::Phone);
    assert_eq!(DeviceClass::detect(false, false), DeviceClass::Desktop);

    for class in [DeviceClass::Desktop, DeviceClass::Phone, DeviceClass::Tablet] {
        assert!(class.display_defaults().is_empty(), "{class:?} has no defaults");
        assert_eq!(class.display_settings(), DisplaySettings::default());
    }
    let epaper = DeviceClass::EpaperTablet.display_settings();
    assert_eq!(epaper.theme, Some(Theme::EInk));
    assert_eq!(epaper.scale_percent, Some(150));
    assert_eq!(epaper.rotate_in_bar, Some(true));
    assert_eq!(epaper.screen_off, None, "a class never sets when screens go off");
    assert!(epaper.screens.is_empty(), "a class never sets a rotation");

    let every_device = DisplaySettings {
        theme: Some(Theme::EInk),
        ..DisplaySettings::default()
    };
    let differing: Vec<String> = DeviceClass::EpaperTablet
        .display_defaults()
        .iter()
        .filter(|default| default.differs(&epaper, &every_device))
        .map(|default| default.name().to_owned())
        .collect();
    assert_eq!(differing, ["Scale", "The rotate button"]);

    let changed = DisplaySettings {
        scale_percent: Some(125),
        ..epaper.clone()
    };
    let desktop = edited(
        &DisplaySettingsContent::new(&changed),
        DeviceClass::Desktop.display_edits(Some(DeviceClass::EpaperTablet), &changed),
    )
    .root();
    assert_eq!(desktop.theme, None, "a default still in place goes with its class");
    assert_eq!(desktop.rotate_in_bar, None);
    assert_eq!(desktop.scale_percent, Some(125), "a value changed since stays");

    let back = edited(
        &DisplaySettingsContent::new(&desktop),
        DeviceClass::EpaperTablet.display_edits(Some(DeviceClass::Desktop), &desktop),
    )
    .root();
    assert_eq!(back, epaper, "picking a class writes its defaults");
}
