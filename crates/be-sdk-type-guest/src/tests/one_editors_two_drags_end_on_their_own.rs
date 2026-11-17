use super::tentative::journaled;
use super::*;

/// One editor holds two drags at once (two fingers), each under its own id: cancelling one
/// puts back only its item, and the other commits as one undo step when it ends.
#[test]
fn one_editors_two_drags_end_on_their_own() {
    let (mut sim, a, b) = super::tentative::dragging(2);
    sim.gesture_in(0, 1, 7, Gesture::BeginTentative);
    sim.gesture_in(0, 1, 8, Gesture::BeginTentative);
    let first = Checklist::set_text(a, "first");
    let second = Checklist::set_text(b, "second");
    assert!(sim.operate_in(0, 1, Some(7), first.clone()));
    assert!(sim.operate_in(0, 1, Some(8), second.clone()));
    sim.settle();
    assert_eq!(texts(sim.mirror(0, 2)), ["first", "second"]);
    assert_eq!(texts(sim.mirror(1, 1)), ["a", "b"], "held from peers");

    sim.gesture_in(0, 1, 8, Gesture::Cancel);
    sim.settle();
    assert_eq!(texts(sim.mirror(0, 2)), ["first", "b"]);
    assert!(!journaled(&sim, &first), "the other drag is still held");

    sim.gesture_in(0, 1, 7, Gesture::Commit);
    sim.settle();
    assert!(journaled(&sim, &first));
    assert!(!journaled(&sim, &second));
    for device in 0..2 {
        assert_eq!(texts(&sim.content(device)), ["first", "b"]);
    }
    assert!(sim.change(0, Change::Undo));
    sim.settle();
    assert_eq!(texts(&sim.content(0)), ["a", "b"]);
}
