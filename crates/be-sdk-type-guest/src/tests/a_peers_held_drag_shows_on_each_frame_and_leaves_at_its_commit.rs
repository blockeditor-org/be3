use super::*;

/// A peer's mirror follows a held drag frame by frame, each frame one landing with no
/// snapshot, while the peer's content holds none of it. What the peer types meanwhile lands
/// beneath the drag, and the commit's entry lands with the layer leaving in that one landing.
#[test]
fn a_peers_held_drag_shows_on_each_frame_and_leaves_at_its_commit() {
    let (mut sim, a, b) = super::tentative::dragging(2);
    sim.gesture(0, 1, Gesture::BeginTentative);
    for frame in ["1", "2"] {
        sim.operate(0, 1, Checklist::set_text(a, frame));
        sim.settle();
        assert_eq!(texts(sim.mirror(1, 1)), [frame, "b"]);
        assert_eq!(texts(sim.mirror(1, 2)), [frame, "b"]);
        let (snapshots, landed) = super::tentative::landings(&mut sim, 1, 2);
        assert_eq!((snapshots, landed.len()), (0, 1));
    }
    assert_eq!(texts(&sim.content(1)), ["a", "b"]);

    sim.operate(1, 1, Checklist::set_text(b, "typed"));
    sim.settle();
    assert_eq!(texts(sim.mirror(1, 2)), ["2", "typed"]);
    assert_eq!(texts(sim.mirror(0, 2)), ["2", "typed"]);
    super::tentative::landings(&mut sim, 1, 2);

    sim.gesture(0, 1, Gesture::Commit);
    sim.settle();
    let (snapshots, landed) = super::tentative::landings(&mut sim, 1, 2);
    assert_eq!(snapshots, 0);
    assert_eq!(landed.len(), 1);
    assert_eq!(landed[0].beneath, Some(Vec::new()));
    assert!(landed[0].operation.is_some());
    for device in 0..2 {
        assert_eq!(texts(&sim.content(device)), ["2", "typed"]);
        assert_eq!(texts(sim.mirror(device, 2)), ["2", "typed"]);
    }
}
