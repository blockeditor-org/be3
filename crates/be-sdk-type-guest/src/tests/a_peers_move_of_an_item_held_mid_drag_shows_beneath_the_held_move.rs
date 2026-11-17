use super::*;

/// A peer changes the item a drag here holds: this device keeps showing the held value on top
/// of it, and once the drag commits every replica ends with the drag's, which the order took
/// last.
#[test]
fn a_peers_move_of_an_item_held_mid_drag_shows_beneath_the_held_move() {
    let (mut sim, a, _) = super::tentative::dragging(2);
    sim.gesture(0, 1, Gesture::BeginTentative);
    sim.operate(0, 1, Checklist::set_text(a, "mine"));
    sim.settle();
    sim.operate(1, 1, Checklist::set_text(a, "theirs"));
    sim.settle();
    assert_eq!(texts(sim.mirror(0, 1)), ["mine", "b"]);
    assert_eq!(texts(sim.mirror(0, 2)), ["mine", "b"]);
    assert_eq!(texts(&sim.content(0)), ["theirs", "b"]);
    assert_eq!(texts(sim.mirror(1, 1)), ["theirs", "b"]);

    sim.gesture(0, 1, Gesture::Commit);
    sim.settle();
    for device in 0..2 {
        assert_eq!(texts(&sim.content(device)), ["mine", "b"]);
        assert_eq!(texts(sim.mirror(device, 1)), ["mine", "b"]);
    }
}
