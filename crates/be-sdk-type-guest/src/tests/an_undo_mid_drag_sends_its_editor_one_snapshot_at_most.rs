use super::tentative::journaled;
use super::*;

/// An undo cancels a held drag while its editor, not yet told, sends several more frames: the
/// replica drops them unanswered and the editor's mirror lets them go when it hears, so the
/// undo costs the editor one snapshot at most, and what it makes after lands as its own.
#[test]
fn an_undo_mid_drag_sends_its_editor_one_snapshot_at_most() {
    let (mut sim, a, b) = super::tentative::dragging(2);
    sim.gesture(0, 1, Gesture::BeginTentative);
    sim.operate(0, 1, Checklist::set_text(a, "held"));
    sim.settle();
    sim.take_received(0, 1);
    sim.deafen(0, 1);
    assert!(sim.change(0, Change::Undo));
    let frames: Vec<Edit> = (0..4)
        .map(|frame| Checklist::set_text(a, &format!("frame {frame}")))
        .collect();
    for frame in &frames {
        assert!(sim.operate_in(0, 1, Some(0), frame.clone()));
    }
    sim.settle();
    sim.hear(0, 1);
    let received = sim.take_received(0, 1);
    let snapshots = received
        .iter()
        .filter(|message| matches!(message, ToEditor::Snapshot(_)))
        .count();
    assert!(snapshots <= 1, "{snapshots} snapshots");
    assert!(received.iter().any(|message| matches!(
        message,
        ToEditor::GestureEnded {
            cancelled: true,
            ..
        }
    )));
    assert_eq!(texts(sim.mirror(0, 1)), ["a", "b"]);
    for frame in &frames {
        assert!(!journaled(&sim, frame));
    }

    // What the editor makes after it heard is answered as its own.
    sim.operate(0, 1, Checklist::set_text(b, "typed"));
    sim.settle();
    assert!(
        !sim.take_received(0, 1)
            .iter()
            .any(|message| matches!(message, ToEditor::Snapshot(_)))
    );
    for device in 0..2 {
        assert_eq!(texts(&sim.content(device)), ["a", "typed"]);
    }
    assert_eq!(texts(sim.mirror(0, 1)), ["a", "typed"]);
    assert_eq!(texts(sim.mirror(0, 2)), ["a", "typed"]);
}
