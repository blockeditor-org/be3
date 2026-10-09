use super::*;

#[test]
fn the_power_menu_locks_the_screen_without_asking() {
    let mut fixture = Fixture::new();
    fixture.allow_power(PowerAvailability {
        lock: false,
        ..EVERYTHING
    });
    fixture.test.click("desktop.power");
    fixture.settle();
    fixture.test.click("desktop.power.lock");
    fixture.settle();
    assert!(
        fixture.test.take_power_requests().is_empty(),
        "a host that cannot lock is not asked to"
    );

    fixture.allow_power(EVERYTHING);
    if !fixture.test.shown("desktop.power.lock") {
        fixture.test.click("desktop.power");
        fixture.settle();
    }
    fixture.test.click("desktop.power.lock");
    fixture.settle();
    fixture.test.settle();
    assert_eq!(fixture.test.take_power_requests(), vec![PowerAction::Lock]);
    assert!(!fixture.test.shown("desktop.power.dialog"));
}
