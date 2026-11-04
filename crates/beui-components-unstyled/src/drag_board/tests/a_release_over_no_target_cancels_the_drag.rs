use super::*;

#[test]
fn a_release_over_no_target_cancels_the_drag() {
    let fixture = Fixture::new();
    fixture
        .target("list", rect(0.0, 0.0, 100.0, 100.0), 1)
        .register();
    fixture.begin(pos2(50.0, 50.0));
    fixture.board.track(at_point(pos2(150.0, 50.0)));
    fixture.log.take();
    fixture.board.finish();
    assert_eq!(
        fixture.log.take(),
        ["list left", "list carrying false", "cancelled"]
    );
    assert!(!fixture.board.carrying());
    assert!(!fixture.board.cancel());
}
