use super::*;

#[test]
fn picking_a_turned_monitor_writes_its_rotation_for_every_device() {
    let mut fixture = Fixture::new();
    let monitor = HostDisplay {
        id: "DEL|U2723QE|1234".to_owned(),
        name: "DELL U2723QE".to_owned(),
        connector: "DP-1".to_owned(),
        ..epaper_panel()
    };
    fixture.report_screens(vec![monitor]);
    let (shared, own) =
        fixture.with_display_settings(rotate_in_bar(), Some(DisplaySettingsContent::default()));
    let own = own.expect("this device has a profile");

    fixture.test.click("desktop.rotate");
    fixture.settle();
    fixture.test.click("desktop.rotate.90");
    fixture.settle();
    assert_eq!(
        fixture
            .test
            .content::<DisplaySettingsContent>(Some(shared))
            .root()
            .rotation("DEL|U2723QE|1234"),
        Rotation::Clockwise90,
        "a monitor known by its serial turns on every device it is plugged into"
    );
    assert_eq!(
        fixture
            .test
            .content::<DisplaySettingsContent>(Some(own))
            .root()
            .screen("DEL|U2723QE|1234")
            .rotation,
        None
    );
}
