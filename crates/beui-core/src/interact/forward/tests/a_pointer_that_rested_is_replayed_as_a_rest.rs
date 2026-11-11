use super::*;

#[test]
fn a_pointer_that_rested_is_replayed_as_a_rest() {
    let mut replay = Replay::default();
    let mut rested = pointer(PointerId::Mouse, PointerPhase::Move, pos2(120.0, 70.0));
    rested.rested = true;
    let moved = pointer(PointerId::Mouse, PointerPhase::Move, pos2(140.0, 90.0));
    assert_eq!(
        replayed(&mut replay, vec![sent(rested), sent(moved)], 2.0),
        [
            Input::PointerRested(pos2(10.0, 10.0)),
            Input::PointerMoved(pos2(20.0, 20.0)),
        ],
        "a pointer that stayed while what is under it changed is not one that moved"
    );
}
