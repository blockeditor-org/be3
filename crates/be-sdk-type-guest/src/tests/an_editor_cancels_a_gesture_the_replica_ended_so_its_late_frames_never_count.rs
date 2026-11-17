use super::tentative::{journaled, told_ended};
use super::*;

/// A peer's replacement ends a drag here that the host cannot see end. The editor, still
/// dragging, sends a frame before it hears, then answers the replica's word with a cancel:
/// the frame is dropped, never counts as a local edit, and leaves nothing in the editor's own
/// view; what the editor makes in the gesture after it heard is not sent at all.
#[test]
fn an_editor_cancels_a_gesture_the_replica_ended_so_its_late_frames_never_count() {
    let (mut sim, a, _) = super::tentative::dragging(2);
    sim.gesture(0, 1, Gesture::BeginTentative);
    sim.operate(0, 1, Checklist::set_text(a, "held"));
    sim.settle();
    sim.take_received(0, 1);
    let taken = sim.devices[0].status.taken;

    sim.deafen(0, 1);
    let mut replacement = ChecklistContent::default();
    let (_, added) = Checklist::add("replaced");
    replacement.apply(&added);
    assert!(sim.change(1, Change::Replace(replacement.encode())));
    sim.settle();
    let late = Checklist::set_text(a, "late");
    assert!(sim.operate_in(0, 1, Some(0), late.clone()));
    sim.settle();
    assert_eq!(sim.devices[0].status.taken, taken, "the late frame is held");

    sim.hear(0, 1);
    assert!(told_ended(&mut sim, 0, 1));
    assert!(
        sim.sent_from(0, 1).contains(&FromEditor::Gesture {
            id: 0,
            gesture: Gesture::Cancel,
        }),
        "the editor answers with a cancel"
    );
    assert_eq!(
        sim.devices[0].status.taken,
        taken,
        "the voided gesture's frames never count"
    );
    assert!(!sim.operate_in(0, 1, Some(0), Checklist::set_text(a, "after")));
    sim.gesture(0, 1, Gesture::Commit);
    sim.settle();
    assert_eq!(sim.devices[0].status.taken, taken);
    assert!(!journaled(&sim, &late));
    for device in 0..2 {
        assert_eq!(texts(&sim.content(device)), ["replaced"]);
    }
    assert_eq!(texts(sim.mirror(0, 1)), ["replaced"]);
}
