use super::*;

#[test]
fn a_band_over_a_full_screen_window_comes_out_only_for_a_long_push() {
    let mut fixture = Fixture::with_windows(&[1]);
    fixture.report_screens(vec![monitor("main", SCREEN, 1.0)]);
    fixture.screen_input(at_foot(0.0, false, false));
    let thick = fixture.test.rect_of("dock.bar.band.bottom").height();
    fixture.show_windows(&[TestWindow {
        fullscreen: Some(SCREEN),
        ..host_window(1, true)
    }]);
    let sent = fixture.sent_input();
    assert!(
        sent.walls.len() == 1,
        "a full screen window hides the bars, which a push brings out: {sent:?}"
    );

    fixture.rest_on_foot(0.0);
    fixture.screen_input(at_foot(thick * 0.9, true, false));
    fixture.wait(16);
    assert!(
        !fixture.test.shown("desktop.bar.status"),
        "a short push shows nothing"
    );
    fixture.screen_input(at_foot(thick * 2.6, true, true));
    fixture.wait(500);
    let status = fixture.test.rect_of("desktop.bar.status");
    assert!(
        (status.bottom() - 600.0).abs() < 1.0,
        "a long push brings the bars out over the window: {status:?}"
    );
    fixture
        .test
        .snapshot("bars_pushed_out_over_a_full_screen_window");
}
