use super::*;

/// A peer that holds another count of a layer than each of several frames appends to asks for
/// the layer whole once, not at every frame.
#[test]
fn a_peer_that_missed_frames_asks_for_the_layer_once() {
    let (mut sim, a, _) = super::tentative::dragging(2);
    sim.gesture(0, 1, Gesture::BeginTentative);
    sim.operate(0, 1, Checklist::set_text(a, "1"));
    sim.settle();
    sim.deaf.insert(1);
    sim.operate(0, 1, Checklist::set_text(a, "2"));
    sim.settle();
    sim.deaf.clear();
    // Its asks go unheard, so it holds another count still at the next frame.
    sim.deaf.insert(0);
    sim.sent.clear();
    for frame in ["3", "4", "5"] {
        sim.operate(0, 1, Checklist::set_text(a, frame));
        sim.settle();
    }
    assert_eq!(super::tentative::asked_again(&sim, 1), 1);
    sim.deaf.clear();
}
