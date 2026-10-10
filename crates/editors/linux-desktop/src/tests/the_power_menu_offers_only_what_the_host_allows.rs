use super::*;

#[test]
fn the_power_menu_offers_only_what_the_host_allows() {
    let mut fixture = Fixture::new();
    fixture.settle();
    assert!(
        fixture.test.watches::<Power>(),
        "the desktop asks the host what it can do"
    );
    fixture.allow_power(PowerAvailability {
        suspend: false,
        ..EVERYTHING
    });

    fixture.test.click("desktop.power");
    fixture.settle();
    fixture.test.settle();
    fixture.test.snapshot("the_power_menu");
    fixture.test.click("desktop.power.suspend");
    fixture.settle();
    assert!(
        fixture.test.take_actions::<PowerAction>().is_empty(),
        "a computer that cannot suspend is not asked to"
    );

    fixture.allow_power(EVERYTHING);
    if !fixture.test.shown("desktop.power.suspend") {
        fixture.test.click("desktop.power");
        fixture.settle();
    }
    fixture.test.click("desktop.power.suspend");
    fixture.settle();
    assert_eq!(
        fixture.test.take_actions::<PowerAction>(),
        vec![PowerAction::Suspend],
        "suspending needs no confirmation"
    );
    assert!(!fixture.test.shown("desktop.power.dialog"));
}
