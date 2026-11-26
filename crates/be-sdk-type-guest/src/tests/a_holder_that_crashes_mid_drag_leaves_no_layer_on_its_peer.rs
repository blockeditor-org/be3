use super::*;

/// The device holding a drag crashes: its peer stops previewing the drag as the holder leaves
/// the session, in one landing, and the order never held it.
#[test]
fn a_holder_that_crashes_mid_drag_leaves_no_layer_on_its_peer() {
    let (mut sim, a, _) = super::tentative::dragging(2);
    sim.gesture(0, 1, Gesture::BeginTentative);
    sim.operate(0, 1, Checklist::set_text(a, "dragged"));
    sim.settle();
    assert_eq!(texts(sim.mirror(1, 1)), ["dragged", "b"]);
    super::tentative::landings(&mut sim, 1, 1);

    sim.crash(0);
    assert_eq!(texts(sim.mirror(1, 1)), ["a", "b"]);
    let (snapshots, landed) = super::tentative::landings(&mut sim, 1, 1);
    assert_eq!((snapshots, landed.len()), (0, 1));
    assert_eq!(texts(&sim.content(1)), ["a", "b"]);
}
