use super::*;

#[test]
fn escape_cancels_a_drag_over_a_target_without_dropping_it() {
    let fixture = Fixture::new();
    fixture.target("list", rect(0.0, 0.0, 100.0, 100.0), 1).register();
    fixture.begin(pos2(50.0, 50.0));
    fixture.log.take();
    assert!(fixture.board.cancel());
    assert_eq!(
        fixture.log.take(),
        ["list left", "list carrying false", "cancelled"]
    );
    assert!(!fixture.board.carrying());
}
