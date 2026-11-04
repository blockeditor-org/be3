use super::*;

#[test]
fn picking_a_turned_screen_writes_its_rotation_into_this_devices_display_settings() {
    let mut fixture = Fixture::new();
    fixture.report_screens(vec![epaper_panel()]);
    let mode = DisplayMode {
        width: 1404,
        height: 1872,
        refresh_millihertz: 85_000,
    };
    let mut own = DisplaySettingsContent::default();
    own.apply(&DisplaySettings::default().set_mode(PANEL, Some(mode)));
    let (shared, own) = fixture.with_display_settings(rotate_in_bar(), Some(own));
    let own = own.expect("this device has a profile");

    fixture.test.click("desktop.rotate");
    fixture.settle();
    assert!(fixture.says("Tap the one that's right side up"));
    fixture.test.snapshot("the_rotate_popover");

    fixture.test.click("desktop.rotate.90");
    fixture.settle();
    let written = fixture
        .test
        .content::<DisplaySettingsContent>(Some(own))
        .root();
    assert_eq!(written.rotation(PANEL), Rotation::Clockwise90);
    assert_eq!(
        written.mode(PANEL),
        Some(mode),
        "the screen's mode stays as it was"
    );
    assert_eq!(
        fixture
            .test
            .content::<DisplaySettingsContent>(Some(shared))
            .root()
            .screen(PANEL)
            .rotation,
        None,
        "a screen known only by its connector is this device's alone"
    );
    assert!(
        !fixture.test.shown("desktop.rotate.90"),
        "picking closes the popover"
    );
}
