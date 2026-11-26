use super::*;

/// Two devices drag at once, one of them the item the other drags too: each shows its own
/// drag on top and the other's beneath it, frame by frame with no snapshot, and the drag
/// committed last is what both end with.
#[test]
fn two_peers_dragging_at_once_each_see_their_own_drag_on_top() {
    let (mut sim, a, b) = super::tentative::dragging(2);
    sim.gesture(1, 1, Gesture::BeginTentative);
    sim.operate(1, 1, Checklist::set_text(a, "theirs"));
    sim.operate(1, 1, Checklist::set_text(b, "moved"));
    sim.settle();
    sim.gesture(0, 1, Gesture::BeginTentative);
    sim.settle();
    super::tentative::landings(&mut sim, 1, 2);
    for frame in ["1", "2"] {
        sim.operate(0, 1, Checklist::set_text(a, frame));
        sim.settle();
        assert_eq!(texts(sim.mirror(0, 2)), [frame, "moved"]);
        assert_eq!(texts(sim.mirror(1, 2)), ["theirs", "moved"]);
        let (snapshots, landed) = super::tentative::landings(&mut sim, 1, 2);
        assert_eq!((snapshots, landed.len()), (0, 1));
        assert!(matches!(landed[0].landed, Landed::Under { above: 2, .. }));
    }

    sim.gesture(1, 1, Gesture::Cancel);
    sim.settle();
    assert_eq!(texts(sim.mirror(1, 2)), ["2", "b"]);
    assert_eq!(texts(sim.mirror(0, 2)), ["2", "b"]);
    sim.gesture(0, 1, Gesture::Commit);
    sim.settle();
    for device in 0..2 {
        assert_eq!(texts(&sim.content(device)), ["2", "b"]);
        assert_eq!(texts(sim.mirror(device, 2)), ["2", "b"]);
    }
}
