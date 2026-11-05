use super::*;

#[test]
fn every_target_hears_the_drag_end_before_the_drop() {
    let fixture = Fixture::new();
    fixture
        .target("first", rect(0.0, 0.0, 100.0, 100.0), 1)
        .register();
    fixture
        .target("second", rect(200.0, 0.0, 100.0, 100.0), 1)
        .register();
    fixture.begin(pos2(50.0, 50.0));
    fixture.log.take();
    assert!(fixture.board.end(Some(at_point(pos2(50.0, 50.0)))));
    assert_eq!(
        fixture.log.take(),
        [
            "first over 50 50",
            "first left",
            "first carrying false",
            "second left",
            "second carrying false",
            "first dropped at 50 50",
            "source dropped",
        ]
    );
}
