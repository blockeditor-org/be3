use super::*;
use block_editor_beui::be_block::ThemeKind;

#[test]
fn devices_and_profiles_shows_only_what_a_profile_keeps() {
    let mut settings = editor();
    settings.set_size(Vec2::new(900.0, 640.0));
    settings.click("settings.devices");
    assert!(
        settings.shown("devices.same"),
        "with no profile keeping anything, the page says so"
    );

    let block = settings.block_id().expect("the settings are a block");
    let (shared, tablet, desktop, tablet_display, desktop_display) = (
        Uuid::from_u128(1),
        Uuid::from_u128(2),
        Uuid::from_u128(3),
        Uuid::from_u128(4),
        Uuid::from_u128(5),
    );
    let store = settings.store();
    for (profile, class, name) in [
        (tablet, DeviceClass::EpaperTablet, "PineNote"),
        (desktop, DeviceClass::Desktop, "Workstation"),
    ] {
        let mut info = BlockInfo::new(
            profile,
            SettingsProfileContent::CONTENT_TYPE,
            BlockParent::Block(block),
        );
        info.set_name(name);
        store.add_block(info);
        let mut own = SettingsProfileContent::default();
        own.apply(&SettingsProfile::set_class(Some(class)));
        own.apply(&SettingsProfile::set_kind(
            DisplaySettingsContent::CONTENT_TYPE,
            match class {
                DeviceClass::EpaperTablet => tablet_display,
                _ => desktop_display,
            },
        ));
        store.hold(Some(profile), own);
    }
    store.hold(
        Some(tablet_display),
        DisplaySettingsContent::new(&DeviceClass::EpaperTablet.display_settings()),
    );
    store.hold(
        Some(desktop_display),
        DisplaySettingsContent::new(&DisplaySettings {
            scale_percent: Some(125),
            ..DisplaySettings::default()
        }),
    );
    store.hold(
        Some(shared),
        DisplaySettingsContent::new(&DisplaySettings {
            theme: Some(ThemeKind::Light),
            ..DisplaySettings::default()
        }),
    );
    let mut held = settings.content::<SettingsContent>(None);
    held.apply(&Settings::set_kind(
        DisplaySettingsContent::CONTENT_TYPE,
        shared,
    ));
    held.apply(&Settings::list_profile(tablet));
    held.apply(&Settings::list_profile(desktop));
    settings.hold(None, held);
    settings.run();

    assert!(!settings.shown("devices.same"));
    assert_eq!(settings.label("devices.theme.all"), "Light");
    assert_eq!(
        settings.label(&format!("devices.theme.{tablet}")),
        "E-paper kept since setup"
    );
    assert_eq!(settings.label(&format!("devices.theme.{desktop}")), "same");
    assert_eq!(settings.label("devices.scale.all"), "not set");
    assert_eq!(
        settings.label(&format!("devices.scale.{tablet}")),
        "150% kept since setup"
    );
    assert_eq!(
        settings.label(&format!("devices.scale.{desktop}")),
        "125% kept on device"
    );
    assert!(settings.shown("devices.rotate-in-bar"));
    assert!(
        !settings.shown("devices.screen-off"),
        "a setting no profile keeps is left out"
    );
    settings.snapshot("devices_and_profiles");

    let mut changed = settings.content::<DisplaySettingsContent>(Some(tablet_display));
    changed.apply(&DisplaySettings::set_scale_percent(Some(175)));
    settings.hold(Some(tablet_display), changed);
    settings.run();
    assert_eq!(
        settings.label(&format!("devices.scale.{tablet}")),
        "175% kept on device",
        "a class default changed on the device is no longer the one from setup"
    );
}
