use super::*;

#[test]
fn a_band_over_a_full_screen_window_comes_out_only_for_a_deliberate_push() {
    let mut fixture = Fixture::with_windows(&[1]);
    fixture.report_screens(vec![monitor("main", SCREEN, 1.0)]);
    fixture.edges(&[]);
    let thick = fixture.test.rect_of("dock.bar.band.bottom").height();
    fixture.show_windows(&[TestWindow {
        fullscreen: Some(SCREEN),
        ..host_window(1, true)
    }]);
    let sent = fixture.sent_edges();
    fixture.edges(&[bottom_edge(0.0, false)]);
    assert!(
        sent.len() == 1 && sent[0].wall && sent[0].deliberate,
        "a full screen window hides the bars, which a deliberate push brings out: {sent:?}"
    );

    fixture.edges(&[bottom_edge(thick * 0.9, false)]);
    fixture.wait(16);
    assert!(
        !fixture.test.shown("desktop.bar.status"),
        "a push that would open an auto-hidden band shows nothing over a full screen window"
    );
    fixture.edges(&[bottom_edge(thick * 2.6, true)]);
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
