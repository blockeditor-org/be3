use super::*;

#[test]
fn a_finger_swiping_in_from_a_hidden_bands_edge_brings_it_out() {
    let mut fixture = Fixture::with_windows(&[1]);
    let thick = fixture.auto_hidden().height();
    let swipe = |pull: Option<f32>, pulls: u32, pulled: f32| ScreenEdgeState {
        pull,
        pulls,
        pulled,
        ..bottom_edge(0.0, false)
    };
    let tapped_away = ScreenEdgeState {
        taps_away: 1,
        ..swipe(None, 2, thick * 0.8)
    };

    fixture.edges(&[swipe(Some(thick * 0.2), 0, 0.0)]);
    fixture.wait(16);
    assert!(
        fixture.test.shown("desktop.bar.status"),
        "the band follows the finger"
    );
    fixture.edges(&[swipe(None, 1, thick * 0.2)]);
    fixture.wait(500);
    assert!(
        !fixture.test.shown("desktop.bar.status"),
        "let go short of half, it slides back"
    );

    fixture.edges(&[swipe(Some(thick * 0.8), 1, thick * 0.2)]);
    fixture.edges(&[swipe(None, 2, thick * 0.8)]);
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
    fixture.edges(&[tapped_away]);
    fixture.wait(500);
    assert!(
        !fixture.test.shown("desktop.bar.status"),
        "until a finger lands elsewhere"
    );
}
