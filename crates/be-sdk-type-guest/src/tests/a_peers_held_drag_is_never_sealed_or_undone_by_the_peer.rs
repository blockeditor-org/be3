use super::*;

/// The peer holding the lease seals mid-drag without the drag, and its own undo steps past its
/// own edit alone: the drag stays on top until its holder cancels it, which leaves nothing.
#[test]
fn a_peers_held_drag_is_never_sealed_or_undone_by_the_peer() {
    let (mut sim, a, b) = super::tentative::dragging(2);
    sim.claim(1);
    sim.settle();
    sim.gesture(0, 1, Gesture::BeginTentative);
    sim.operate(0, 1, Checklist::set_text(a, "dragged"));
    sim.settle();

    sim.operate(1, 1, Checklist::set_text(b, "typed"));
    sim.settle();
    assert_eq!(texts(sim.mirror(1, 1)), ["dragged", "typed"]);
    sim.advance(crate::QUIET_INTERVAL + 1);
    sim.settle();
    assert_eq!(texts(&sim.head_content()), ["a", "typed"]);

    assert!(sim.change(1, Change::Undo));
    sim.settle();
    assert_eq!(texts(sim.mirror(1, 1)), ["dragged", "b"]);
    assert_eq!(texts(&sim.content(1)), ["a", "b"]);

    sim.gesture(0, 1, Gesture::Cancel);
    sim.settle();
    for device in 0..2 {
        assert_eq!(texts(&sim.content(device)), ["a", "b"]);
        assert_eq!(texts(sim.mirror(device, 1)), ["a", "b"]);
    }
}
