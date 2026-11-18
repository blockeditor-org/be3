use super::tentative::{dragging, journaled};
use super::*;

/// While one editor holds a drag, another types: the typed edit goes out at once, beneath the
/// held one, and the drag follows on its commit.
#[test]
fn a_typed_edit_during_a_held_drag_reaches_peers_at_once() {
    let (mut sim, a, b) = dragging(2);
    let dragged = Checklist::set_text(a, "dragged");
    sim.gesture(0, 1, Gesture::BeginTentative);
    sim.operate(0, 1, dragged.clone());
    sim.settle();
    sim.operate(0, 2, Checklist::set_text(b, "typed"));
    let (_, added) = Checklist::add("added");
    sim.operate(0, 2, added);
    sim.settle();
    assert_eq!(texts(&sim.content(1)), ["a", "typed", "added"]);
    assert_eq!(texts(sim.mirror(1, 1)), ["a", "typed", "added"]);
    assert_eq!(texts(sim.mirror(0, 2)), ["dragged", "typed", "added"]);
    assert!(!journaled(&sim, &dragged));

    sim.gesture(0, 1, Gesture::Commit);
    sim.settle();
    for device in 0..2 {
        assert_eq!(texts(&sim.content(device)), ["dragged", "typed", "added"]);
    }

    // The drag is its own undo step, after the typing.
    assert!(sim.change(0, Change::Undo));
    sim.settle();
    assert_eq!(texts(&sim.content(0)), ["a", "typed", "added"]);
}
