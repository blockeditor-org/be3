use super::*;
use crate::{DeviceClass, Motion, ThemeKind};

#[test]
fn a_device_class_is_detected_and_brings_its_own_defaults() {
    assert_eq!(DeviceClass::detect(true, false), DeviceClass::EpaperTablet);
    assert_eq!(DeviceClass::detect(true, true), DeviceClass::EpaperTablet);
    assert_eq!(DeviceClass::detect(false, true), DeviceClass::Phone);
    assert_eq!(DeviceClass::detect(false, false), DeviceClass::Desktop);

    for class in [DeviceClass::Desktop, DeviceClass::Phone] {
        assert!(
            class.display_defaults().is_empty(),
            "{class:?} has no defaults"
        );
        assert_eq!(class.display_settings(), DisplaySettings::default());
    }
    let tablet = DeviceClass::Tablet.display_settings();
    assert!(tablet.rotate_in_bar() && tablet.keyboard_in_bar());
    assert_eq!(
        tablet,
        DisplaySettings {
            rotate_in_bar: Some(true),
            keyboard_in_bar: Some(true),
            ..DisplaySettings::default()
        },
        "a tablet only puts its device tools on the bar"
    );
    let epaper = DeviceClass::EpaperTablet.display_settings();
    assert_eq!(epaper.theme, Some(ThemeKind::EPaper));
    assert_eq!(epaper.motion, Some(Motion::Instant));
    assert_eq!(epaper.scale_percent, Some(150));
    assert_eq!(epaper.rotate_in_bar, Some(true));
    assert_eq!(epaper.keyboard_in_bar, Some(true));
    assert_eq!(
        epaper.screen_off, None,
        "a class never sets when screens go off"
    );
    assert!(epaper.screens.is_empty(), "a class never sets a rotation");

    let every_device = DisplaySettings {
        theme: Some(ThemeKind::EPaper),
        ..DisplaySettings::default()
    };
    let differing: Vec<String> = DeviceClass::EpaperTablet
        .display_defaults()
        .iter()
        .filter(|default| default.differs(&epaper, &every_device))
        .map(|default| default.name().to_owned())
        .collect();
    assert_eq!(
        differing,
        [
            "Motion",
            "Scale",
            "The rotate button",
            "The keyboard button"
        ]
    );

    let changed = DisplaySettings {
        scale_percent: Some(125),
        ..epaper.clone()
    };
    let desktop = edited(
        &DisplaySettingsContent::new(&changed),
        DeviceClass::Desktop.display_edits(Some(DeviceClass::EpaperTablet), &changed),
    )
    .root();
    assert_eq!(
        desktop.theme, None,
        "a default still in place goes with its class"
    );
    assert_eq!(desktop.rotate_in_bar, None);
    assert_eq!(desktop.keyboard_in_bar, None);
    assert_eq!(desktop.motion, None);
    assert_eq!(
        desktop.scale_percent,
        Some(125),
        "a value changed since stays"
    );

    let back = edited(
        &DisplaySettingsContent::new(&desktop),
        DeviceClass::EpaperTablet.display_edits(Some(DeviceClass::Desktop), &desktop),
    )
    .root();
    assert_eq!(back, epaper, "picking a class writes its defaults");

    let kept = DisplaySettings {
        keyboard_in_bar: Some(false),
        ..epaper.clone()
    };
    let tablet_again = edited(
        &DisplaySettingsContent::new(&kept),
        DeviceClass::Tablet.display_edits(Some(DeviceClass::EpaperTablet), &kept),
    )
    .root();
    assert_eq!(
        tablet_again, tablet,
        "the new class's defaults are written over the old class's"
    );
}
