use super::*;

#[test]
fn the_power_row_offers_only_what_the_host_allows() {
    let mut fixture = Fixture::new();
    fixture.settle();
    assert!(
        fixture.test.watches::<PowerWatch>(),
        "the desktop asks the host what it can do"
    );
    fixture.allow_power(PowerAvailability {
        suspend: false,
        ..EVERYTHING
    });

    fixture.wifi(nearby());
    fixture.hear(0.45, false, SPEAKERS);
    assert!(fixture.test.shown("desktop.status.power"));

    fixture.test.click("desktop.status");
    fixture.settle();
    fixture.test.settle();
    assert!(fixture.test.shown("desktop.quick.wifi"));
    assert!(fixture.test.shown("desktop.volume.panel"));
    assert!(fixture.test.shown("desktop.power.lock"));
    fixture.test.snapshot("the_quick_settings");
    fixture.test.click("desktop.power.suspend");
    fixture.settle();
    assert!(
        fixture
            .test
            .take_host_calls(block_ui_test::requested::power)
            .is_empty(),
        "a computer that cannot suspend is not asked to"
    );

    fixture.allow_power(EVERYTHING);
    if !fixture.test.shown("desktop.power.suspend") {
        fixture.test.click("desktop.status");
        fixture.settle();
    }
    fixture.test.click("desktop.power.suspend");
    fixture.settle();
    assert_eq!(
        fixture
            .test
            .take_host_calls(block_ui_test::requested::power),
        vec![PowerAction::Suspend],
        "suspending needs no confirmation"
    );
    assert!(!fixture.test.shown("desktop.power.dialog"));
}
