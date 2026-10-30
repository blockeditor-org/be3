use super::*;

#[test]
fn quick_settings_offers_the_keyboard_rotation_and_refresh_the_screen_has() {
    let mut fixture = Fixture::new();
    fixture.settle();
    fixture.test.click("desktop.status");
    fixture.settle();
    assert!(fixture.test.shown("desktop.quick.keyboard"));
    for absent in ["desktop.quick.rotate", "desktop.quick.refresh"] {
        assert!(
            !fixture.test.shown(absent),
            "{absent} waits for a screen that has it"
        );
    }
    fixture.test.click("desktop.quick.keyboard");
    fixture.settle();
    assert!(
        !fixture.test.shown("desktop.quick"),
        "showing the keyboard closes quick settings"
    );

    fixture.report_screens(vec![epaper_panel()]);
    let (_, own) = fixture.with_display_settings(
        DisplaySettingsContent::default(),
        Some(DisplaySettingsContent::default()),
    );
    let own = own.expect("this device has a profile");
    fixture.test.set_watched::<EpaperWatch>(&true);
    fixture.settle();
    fixture.test.click("desktop.status");
    fixture.settle();
    fixture.test.settle();
    assert!(fixture.test.shown("desktop.quick.refresh"));
    fixture.test.snapshot("quick_settings_on_an_epaper_screen");

    fixture.test.click("desktop.quick.refresh");
    fixture.settle();
    assert_eq!(
        fixture
            .test
            .take_host_calls(block_ui_test::requested::epaper_refresh),
        vec![()]
    );
    assert!(!fixture.test.shown("desktop.quick"));

    fixture.test.click("desktop.status");
    fixture.settle();
    fixture.test.click("desktop.quick.rotate");
    fixture.settle();
    assert!(fixture.says("Tap the one that's right side up"));
    fixture.test.click("desktop.quick.rotate.90");
    fixture.settle();
    assert_eq!(
        fixture
            .test
            .content::<DisplaySettingsContent>(Some(own))
            .root()
            .rotation(PANEL),
        Rotation::Clockwise90
    );
    assert!(!fixture.test.shown("desktop.quick"), "picking closes it");

    fixture.test.click("desktop.status");
    fixture.settle();
    assert!(
        fixture.test.shown("desktop.quick.rotate"),
        "it opens on the tiles again"
    );
}
