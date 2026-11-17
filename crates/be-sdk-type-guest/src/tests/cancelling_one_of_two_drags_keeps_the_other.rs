use super::tentative::{dragging, journaled};
use super::*;

/// Of two drags held in one block, one is cancelled: its edit leaves the preview, the other
/// stays held, and only the other is written when it commits.
#[test]
fn cancelling_one_of_two_drags_keeps_the_other() {
    let (mut sim, a, b) = dragging(1);
    let (first, second) = (Checklist::set_text(a, "first"), Checklist::set_text(b, "second"));
    sim.gesture(0, 1, Gesture::BeginTentative);
    sim.operate(0, 1, first.clone());
    sim.gesture(0, 2, Gesture::BeginTentative);
    sim.operate(0, 2, second.clone());
    sim.gesture(0, 1, Gesture::Cancel);
    sim.settle();
    assert_eq!(texts(sim.mirror(0, 2)), ["a", "second"]);
    assert_eq!(texts(&sim.content(0)), ["a", "b"]);

    sim.gesture(0, 2, Gesture::Commit);
    sim.settle();
    assert!(!journaled(&sim, &first));
    assert!(journaled(&sim, &second));
    assert_eq!(texts(&sim.content(0)), ["a", "second"]);
    assert_eq!(texts(sim.mirror(0, 1)), ["a", "second"]);
}
