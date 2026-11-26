use super::*;

#[test]
fn a_finger_swiping_in_from_a_hidden_bands_edge_brings_it_out() {
    let mut fixture = Fixture::with_windows(&[1]);
    let thick = fixture.auto_hidden().height();
    let at = |pulled: f32| pos2(400.0, 600.0 - pulled);
    let swipe = |touch_downs: u32, touch: Option<f32>, lifts: u32, lifted: f32| {
        let mut state = at_foot(0.0, false, false);
        state.touch_downs = touch_downs;
        state.regions[0] = ScreenRegionState {
            touch_downs,
            touch: touch.map(at),
            lifts,
            lifted: (lifts > 0).then(|| at(lifted)),
            ..foot_region(FOOT_SWIPE)
        };
        state
    };

    fixture.screen_input(swipe(1, Some(thick * 0.2), 0, 0.0));
    fixture.wait(16);
    assert!(
        fixture.test.shown("desktop.bar.status"),
        "the band follows the finger"
    );
    fixture.screen_input(swipe(1, None, 1, thick * 0.2));
    fixture.wait(500);
    assert!(
        !fixture.test.shown("desktop.bar.status"),
        "let go short of half, it slides back"
    );

    fixture.screen_input(swipe(2, Some(thick * 0.8), 1, thick * 0.2));
    fixture.screen_input(swipe(2, None, 2, thick * 0.8));
    fixture.wait(500);
    let status = fixture.test.rect_of("desktop.bar.status");
    assert!(
        (status.bottom() - 600.0).abs() < 1.0,
        "let go past half, it comes all the way out: {status:?}"
    );
    fixture.wait(1000);
    assert!(
        fixture.test.shown("desktop.bar.status"),
        "with no pointer to go away, it stays"
    );
    let mut tapped_away = swipe(2, None, 2, thick * 0.8);
    tapped_away.touch_downs = 3;
    fixture.screen_input(tapped_away);
    fixture.wait(500);
    assert!(
        !fixture.test.shown("desktop.bar.status"),
        "until a finger lands elsewhere"
    );
}
