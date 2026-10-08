use super::*;

#[test]
fn the_power_menu_asks_before_ending_the_session() {
    let mut fixture = Fixture::new();
    fixture.allow_power(EVERYTHING);

    for (item, action) in [
        ("desktop.power.restart", PowerAction::Restart),
        ("desktop.power.power-off", PowerAction::PowerOff),
        ("desktop.power.log-out", PowerAction::LogOut),
    ] {
        fixture.test.click("desktop.power");
        fixture.settle();
        fixture.test.click(item);
        fixture.settle();
        fixture.test.settle();
        assert!(
            fixture.test.shown("desktop.power.dialog"),
            "{item} asks first"
        );
        assert!(
            fixture.test.take_power_requests().is_empty(),
            "nothing happens before {item} is confirmed"
        );
        if action == PowerAction::PowerOff {
            fixture.test.snapshot("powering_off_asks_first");
        }

        fixture.test.click("desktop.power.cancel");
        fixture.settle();
        fixture.test.settle();
        assert!(
            fixture.test.take_power_requests().is_empty(),
            "cancelling does nothing"
        );

        fixture.test.click("desktop.power");
        fixture.settle();
        fixture.test.click(item);
        fixture.settle();
        fixture.test.settle();
        fixture.test.click("desktop.power.confirm");
        fixture.settle();
        fixture.test.settle();
        assert_eq!(fixture.test.take_power_requests(), vec![action]);
    }
}
