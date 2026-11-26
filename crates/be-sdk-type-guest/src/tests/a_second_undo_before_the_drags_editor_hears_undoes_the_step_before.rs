use super::tentative::{dragging, journaled};
use super::*;

/// An undo cancels a held drag; before the drag's editor hears and ends it, a second undo
/// takes back the step before and a redo makes it again: the voided drag holds nothing back.
#[test]
fn a_second_undo_before_the_drags_editor_hears_undoes_the_step_before() {
    let (mut sim, a, b) = dragging(2);
    sim.operate(0, 2, Checklist::set_text(b, "typed"));
    sim.settle();
    sim.advance(1_000);
    sim.gesture(0, 1, Gesture::BeginTentative);
    let held = Checklist::set_text(a, "held");
    sim.operate(0, 1, held.clone());
    sim.settle();
    sim.deafen(0, 1);

    assert!(sim.change(0, Change::Undo));
    assert_eq!(texts(&sim.content(0)), ["a", "typed"]);
    assert!(sim.change(0, Change::Undo));
    assert_eq!(texts(&sim.content(0)), ["a", "b"]);
    assert!(sim.change(0, Change::Redo));
    assert_eq!(texts(&sim.content(0)), ["a", "typed"]);
    let late = Checklist::set_text(a, "late");
    assert!(sim.operate_in(0, 1, Some(0), late.clone()));
    sim.settle();
    sim.hear(0, 1);

    assert!(!journaled(&sim, &held));
    assert!(!journaled(&sim, &late));
    for device in 0..2 {
        assert_eq!(texts(&sim.content(device)), ["a", "typed"]);
    }
    for (device, editor) in [(0, 1), (0, 2), (1, 1)] {
        assert_eq!(texts(sim.mirror(device, editor)), ["a", "typed"]);
    }
}
