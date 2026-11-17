use super::tentative::{journaled, told_ended};
use super::*;

/// An undo cancels a held drag while its editor, not yet told, goes on dragging: what it sends
/// until it ends the gesture is dropped, whether it then commits or cancels, here and on peers.
#[test]
fn frames_a_drag_sends_after_an_undo_cancelled_it_write_nothing() {
    for ending in [Gesture::Commit, Gesture::Cancel] {
        let (mut sim, a, _) = super::tentative::dragging(2);
        sim.gesture(0, 1, Gesture::BeginTentative);
        sim.operate(0, 1, Checklist::set_text(a, "held"));
        sim.settle();
        sim.take_received(0, 1);
        assert!(sim.change(0, Change::Undo));
        let later = Checklist::set_text(a, "later");
        sim.operate(0, 1, later.clone());
        sim.gesture(0, 1, ending);
        sim.settle();
        assert!(told_ended(&mut sim, 0, 1));
        assert!(!journaled(&sim, &later), "{ending:?}");
        for device in 0..2 {
            assert_eq!(texts(&sim.content(device)), ["a", "b"], "{ending:?}");
        }
        assert_eq!(texts(sim.mirror(0, 2)), ["a", "b"]);
        assert_eq!(texts(sim.mirror(1, 1)), ["a", "b"]);
    }
}
