use super::tentative::{journaled, told_ended};
use super::*;

/// An undo cancels a held drag while its editor, not yet told, goes on dragging: what it sends
/// until it hears is dropped, whether it ends the gesture first (a commit or a cancel crossing
/// the replica's word) or the cancel it answers with ends it, here and on peers.
#[test]
fn frames_a_drag_sends_after_an_undo_cancelled_it_write_nothing() {
    for ending in [Some(Gesture::Commit), Some(Gesture::Cancel), None] {
        let (mut sim, a, _) = super::tentative::dragging(2);
        sim.gesture(0, 1, Gesture::BeginTentative);
        sim.operate(0, 1, Checklist::set_text(a, "held"));
        sim.settle();
        sim.take_received(0, 1);
        sim.deafen(0, 1);
        assert!(sim.change(0, Change::Undo));
        let later = Checklist::set_text(a, "later");
        assert!(sim.operate_in(0, 1, Some(0), later.clone()));
        if let Some(ending) = ending {
            sim.gesture(0, 1, ending);
        }
        sim.settle();
        sim.hear(0, 1);
        assert!(told_ended(&mut sim, 0, 1));
        assert!(!journaled(&sim, &later), "{ending:?}");
        for device in 0..2 {
            assert_eq!(texts(&sim.content(device)), ["a", "b"], "{ending:?}");
        }
        for (device, editor) in [(0, 1), (0, 2), (1, 1)] {
            assert_eq!(texts(sim.mirror(device, editor)), ["a", "b"], "{ending:?}");
        }
    }
}
