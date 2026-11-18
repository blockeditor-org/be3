use super::*;

#[test]
fn an_auto_hidden_band_stays_out_while_a_menu_from_its_grip_is_open() {
    let mut fixture = Fixture::with_windows(&[1]);
    let thick = fixture.auto_hidden().height();
    fixture.rest_on_foot(0.0);
    fixture.screen_input(at_foot(thick * 3.0, true, true));
    fixture.wait(500);
    assert!(fixture.test.shown("desktop.bar.status"));

    fixture.bar_menu(STATUS_BAR, &[]);
    assert!(
        fixture.test.shown("grip.dock.bar.auto-hide"),
        "the grip opens its menu"
    );
    fixture.screen_input(at_foot(thick * 3.0, false, false));
    fixture.wait(1000);
    assert!(
        fixture.test.shown("desktop.bar.status") && fixture.test.shown("grip.dock.bar.auto-hide"),
        "the pointer leaving the band for the menu over it leaves both out"
    );

    fixture.test.key(Key::Escape, Modifiers::NONE);
    fixture.wait(500);
    assert!(
        !fixture.test.shown("grip.dock.bar.auto-hide") && !fixture.test.shown("desktop.bar.status"),
        "once the menu closes, the band goes with the pointer away"
    );
}
