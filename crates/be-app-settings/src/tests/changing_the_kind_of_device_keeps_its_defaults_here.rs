use super::*;

#[test]
fn changing_the_kind_of_device_keeps_its_defaults_here() {
    let mut setup = SetUp::new(DeviceClass::Desktop, DisplaySettings::default());
    assert!(
        !setup.shows("setup.done"),
        "a desktop has nothing to set up"
    );
    assert_eq!(setup.here(), DisplaySettings::default());

    setup.choose_kind(DeviceClass::EpaperTablet);
    assert_eq!(setup.profile().class, Some(DeviceClass::EpaperTablet));
    assert_eq!(
        setup.here(),
        DeviceClass::EpaperTablet.display_settings(),
        "the profile makes display settings of its own with the kind's defaults"
    );
    assert!(setup.shows("setup.done"), "the same view sets it up");

    setup.choose_kind(DeviceClass::Tablet);
    assert_eq!(setup.profile().class, Some(DeviceClass::Tablet));
    assert_eq!(
        setup.here(),
        DeviceClass::Tablet.display_settings(),
        "the defaults the old kind kept go with it, and the new kind's take their place"
    );
    assert!(setup.here().keyboard_in_bar() && setup.here().rotate_in_bar());
}
