use super::*;

/// The peer misses a frame of a second drag while the first is committing: it stops showing
/// the second until it is told it whole, and keeps showing the first until the order holds it.
#[test]
fn a_missed_frame_of_a_second_drag_keeps_the_first_committing_beneath_it() {
    let (mut sim, a, b) = super::tentative::dragging(2);
    sim.gesture(0, 1, Gesture::BeginTentative);
    sim.operate(0, 1, Checklist::set_text(a, "moved"));
    sim.settle();
    sim.stalled_from = Some(0);
    sim.gesture(0, 1, Gesture::Commit);
    sim.settle();
    sim.gesture(0, 1, Gesture::BeginTentative);
    sim.operate(0, 1, Checklist::set_text(b, "1"));
    sim.settle();
    sim.deaf.insert(1);
    sim.operate(0, 1, Checklist::set_text(b, "2"));
    sim.settle();
    sim.deaf.clear();

    // Asked for whole, which is answered at once: the first stays beneath.
    sim.operate(0, 1, Checklist::set_text(b, "3"));
    sim.settle();
    assert_eq!(texts(sim.mirror(1, 1)), ["moved", "3"]);
    assert_eq!(texts(&sim.content(1)), ["a", "b"]);

    sim.stalled_from = None;
    sim.unstall();
    assert_eq!(texts(&sim.content(1)), ["moved", "b"]);
    assert_eq!(texts(sim.mirror(1, 1)), ["moved", "3"]);
}
