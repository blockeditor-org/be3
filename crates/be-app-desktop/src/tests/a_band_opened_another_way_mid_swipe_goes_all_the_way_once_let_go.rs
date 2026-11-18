use super::*;

#[test]
fn a_band_opened_another_way_mid_swipe_goes_all_the_way_once_let_go() {
    let mut fixture = Fixture::with_windows(&[1]);
    let thick = fixture.auto_hidden().height();
    let mut swiping = at_foot(0.0, false, false);
    swiping.touch_downs = 1;
    swiping.regions[0] = ScreenRegionState {
        touch_downs: 1,
        touch: Some(pos2(400.0, 600.0 - thick * 0.3)),
        ..foot_region(FOOT_SWIPE)
    };
    fixture.screen_input(swiping);
    fixture.wait(16);
    assert!(
        fixture.test.shown("desktop.bar.status"),
        "the band follows the finger"
    );

    assert!(fixture.test.host_tap(Key::Logo));
    fixture.settle();
    assert!(
        fixture.test.shown("desktop.apps.panel"),
        "Super opens the Apps panel, holding the band out"
    );
    // The open band asks for no swipe strip, so the host drops it and the finger it held: the
    // desktop never hears that finger lift.
    let mut open = at_foot(0.0, false, false);
    open.touch_downs = 1;
    open.regions.retain(|region| region.id != FOOT_SWIPE);
    fixture.screen_input(open);
    fixture.wait(500);

    fixture.test.key(Key::Escape, Modifiers::NONE);
    fixture.wait(500);
    assert!(
        !fixture.test.shown("desktop.apps.panel") && !fixture.test.shown("desktop.bar.status"),
        "once nothing holds it, it goes all the way, not back to where the finger had it"
    );
}
