use super::*;

/// A second drag begins before the first one's commit lands: the peer keeps showing the first
/// beneath the second until the order holds it, never what was there before either.
#[test]
fn a_drag_begun_before_the_last_ones_commit_lands_leaves_that_commit_showing() {
    let (mut sim, a, b) = super::tentative::dragging(2);
    sim.gesture(0, 1, Gesture::BeginTentative);
    sim.operate(0, 1, Checklist::set_text(a, "moved"));
    sim.settle();
    sim.stalled_from = Some(0);
    sim.gesture(0, 1, Gesture::Commit);
    sim.settle();
    sim.gesture(0, 1, Gesture::BeginTentative);
    sim.operate(0, 1, Checklist::set_text(b, "second"));
    sim.settle();
    assert_eq!(texts(sim.mirror(1, 1)), ["moved", "second"]);
    assert_eq!(texts(&sim.content(1)), ["a", "b"]);

    sim.stalled_from = None;
    sim.unstall();
    assert_eq!(texts(&sim.content(1)), ["moved", "b"]);
    assert_eq!(texts(sim.mirror(1, 1)), ["moved", "second"]);
    sim.gesture(0, 1, Gesture::Cancel);
    sim.settle();
    assert_eq!(texts(sim.mirror(1, 1)), ["moved", "b"]);
}
