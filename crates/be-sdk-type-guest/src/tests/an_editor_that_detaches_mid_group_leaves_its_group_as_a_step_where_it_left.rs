use super::tentative::dragging;
use super::*;

/// An editor detaches with an undo group open and no commit for it: the detach commits the
/// group as one step there, so an undo takes back what was made after it first.
#[test]
fn an_editor_that_detaches_mid_group_leaves_its_group_as_a_step_where_it_left() {
    let (mut sim, a, b) = dragging(1);
    sim.gesture(0, 1, Gesture::Begin);
    sim.operate(0, 1, Checklist::set_text(a, "grouped"));
    sim.operate(0, 1, Checklist::set_text(a, "grouped twice"));
    sim.settle();
    sim.detach(0, 1);
    sim.settle();
    sim.advance(1_000);
    sim.operate(0, 2, Checklist::set_text(b, "typed"));
    sim.settle();

    assert!(sim.change(0, Change::Undo));
    sim.settle();
    assert_eq!(texts(&sim.content(0)), ["grouped twice", "b"]);
    assert!(sim.change(0, Change::Undo));
    sim.settle();
    assert_eq!(texts(&sim.content(0)), ["a", "b"]);
}
