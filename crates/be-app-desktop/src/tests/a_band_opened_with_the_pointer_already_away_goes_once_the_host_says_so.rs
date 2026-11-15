use super::*;

/// What the host says before it has heard of the region around the open band.
fn unasked(mut state: ScreenInputState) -> ScreenInputState {
    state.regions.retain(|region| region.id != FOOT_KEEP);
    state
}

#[test]
fn a_band_opened_with_the_pointer_already_away_goes_once_the_host_says_so() {
    let mut fixture = Fixture::with_windows(&[1]);
    let thick = fixture.auto_hidden().height();
    fixture.screen_input(unasked(at_foot(0.0, true, false)));
    fixture.wait(200);
    fixture.screen_input(unasked(at_foot(thick * 3.0, true, false)));
    fixture.wait(500);
    assert!(
        fixture.test.shown("desktop.bar.status"),
        "a long push brings it out, and it stays until the host says where the pointer is"
    );

    fixture.screen_input(at_foot(thick * 3.0, false, false));
    fixture.wait(500);
    assert!(
        !fixture.test.shown("desktop.bar.status"),
        "the pointer left before the host heard of the band's region, so it goes when first told"
    );
}
