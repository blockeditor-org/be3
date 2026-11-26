use super::tentative::{journaled, told_ended};
use super::*;

/// Undo while a drag is held cancels the drag, as Escape would, and does nothing else: the
/// drag's editor is told, its commit then writes nothing, and the next undo takes back the edit
/// before the drag.
#[test]
fn undo_mid_drag_cancels_it() {
    let (mut sim, a, b) = super::tentative::dragging(1);
    sim.operate(0, 2, Checklist::set_text(b, "typed"));
    sim.settle();
    let held = Checklist::set_text(a, "held");
    sim.gesture(0, 1, Gesture::BeginTentative);
    sim.operate(0, 1, held.clone());
    sim.settle();
    sim.take_received(0, 1);
    assert!(sim.devices[0].undo.can_undo);

    assert!(sim.change(0, Change::Undo));
    sim.settle();
    assert!(told_ended(&mut sim, 0, 1));
    assert_eq!(texts(sim.mirror(0, 2)), ["a", "typed"]);
    assert_eq!(texts(&sim.content(0)), ["a", "typed"]);

    sim.gesture(0, 1, Gesture::Commit);
    sim.settle();
    assert!(!journaled(&sim, &held));
    assert_eq!(texts(&sim.content(0)), ["a", "typed"]);

    assert!(sim.change(0, Change::Undo));
    sim.settle();
    assert_eq!(texts(&sim.content(0)), ["a", "b"]);
}
