use super::*;

#[test]
fn an_auto_hidden_band_comes_out_past_half_a_push_and_goes_once_the_pointer_leaves() {
    let mut fixture = Fixture::with_windows(&[1]);
    let band = fixture.auto_hidden();
    let thick = band.height();
    assert!(
        !fixture.test.shown("desktop.bar.status") && !fixture.test.shown("desktop.apps"),
        "the auto-hidden band is gone from the foot"
    );
    assert_eq!(
        fixture.sent_edges(),
        vec![ScreenEdge {
            id: 1,
            display: SCREEN,
            side: ScreenSide::Bottom,
            start: 0.0,
            end: 800.0,
            wall: true,
            deliberate: false,
            keep: None,
        }],
        "the host walls the pointer along the foot"
    );

    fixture.edges(&[bottom_edge(thick * 0.3, false)]);
    fixture.wait(16);
    assert!(
        fixture.test.shown("desktop.bar.status"),
        "a short push brings the band partly out"
    );
    fixture
        .test
        .snapshot("an_auto_hidden_band_partly_pushed_out");
    fixture.wait(1000);
    assert!(
        !fixture.test.shown("desktop.bar.status"),
        "short of half, the band slides back once the push stops"
    );

    fixture.edges(&[bottom_edge(thick * 0.9, true)]);
    fixture.wait(500);
    let status = fixture.test.rect_of("desktop.bar.status");
    assert!(
        (status.bottom() - 600.0).abs() < 1.0,
        "past half, the band comes all the way out: {status:?}"
    );
    let open = fixture.sent_edges();
    assert!(
        !open[0].wall && open[0].keep.is_some_and(|keep| keep.contains_rect(band)),
        "the open band lets the pointer through and keeps it near: {open:?}"
    );
    fixture.test.snapshot("an_auto_hidden_band_pushed_out");

    fixture.wait(1000);
    assert!(
        fixture.test.shown("desktop.bar.status"),
        "it stays while the pointer is near"
    );
    fixture.edges(&[bottom_edge(thick * 0.9, false)]);
    fixture.wait(300);
    assert!(
        fixture.test.shown("desktop.bar.status"),
        "it waits a moment"
    );
    fixture.wait(1000);
    assert!(
        !fixture.test.shown("desktop.bar.status"),
        "and goes once the pointer has been away"
    );
    assert!(fixture.sent_edges()[0].wall, "the wall is back");
}
