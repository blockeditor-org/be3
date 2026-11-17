use super::*;

/// The holder seals while it holds a drag: the commit holds what was typed, not the drag, and
/// the drag reaches the next commit once it commits.
#[test]
fn a_seal_mid_drag_leaves_the_drag_out_of_the_commit() {
    let (mut sim, a, b) = super::tentative::dragging(1);
    sim.gesture(0, 1, Gesture::BeginTentative);
    sim.operate(0, 1, Checklist::set_text(a, "held"));
    sim.operate(0, 2, Checklist::set_text(b, "typed"));
    sim.settle();
    let before = sim.publishes;
    sim.advance(crate::QUIET_INTERVAL);
    sim.settle();
    assert!(sim.publishes > before, "the holder sealed");
    assert_eq!(texts(&sim.head_content()), ["a", "typed"]);
    assert_eq!(texts(sim.mirror(0, 1)), ["held", "typed"]);

    sim.gesture(0, 1, Gesture::Commit);
    sim.settle();
    sim.advance(crate::QUIET_INTERVAL);
    sim.settle();
    assert_eq!(texts(&sim.head_content()), ["held", "typed"]);
}
