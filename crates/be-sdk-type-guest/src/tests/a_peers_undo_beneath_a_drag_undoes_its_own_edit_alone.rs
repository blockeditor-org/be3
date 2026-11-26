use super::*;

/// A peer edits the item another device drags, then undoes it: the undo takes back its own
/// edit beneath the drag, which stays on top, and redo puts it back.
#[test]
fn a_peers_undo_beneath_a_drag_undoes_its_own_edit_alone() {
    let (mut sim, a, _) = super::tentative::dragging(2);
    sim.gesture(0, 1, Gesture::BeginTentative);
    sim.operate(0, 1, Checklist::set_text(a, "dragged"));
    sim.settle();
    sim.operate(1, 1, Checklist::set_text(a, "typed"));
    sim.settle();
    assert_eq!(texts(&sim.content(1)), ["typed", "b"]);

    assert!(sim.change(1, Change::Undo));
    sim.settle();
    assert_eq!(texts(&sim.content(1)), ["a", "b"]);
    assert_eq!(texts(sim.mirror(1, 1)), ["dragged", "b"]);
    assert!(sim.change(1, Change::Redo));
    sim.settle();
    assert_eq!(texts(&sim.content(1)), ["typed", "b"]);

    sim.gesture(0, 1, Gesture::Cancel);
    sim.settle();
    for device in 0..2 {
        assert_eq!(texts(&sim.content(device)), ["typed", "b"]);
    }
}
