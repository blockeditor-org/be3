use super::tentative::dragging;
use super::*;

/// Whether `editor` on `device` was sent a snapshot since last asked.
fn snapshotted(sim: &mut Sim<ChecklistContent>, device: usize, editor: EditorId) -> bool {
    sim.take_received(device, editor)
        .iter()
        .any(|message| matches!(message, ToEditor::Snapshot(_)))
}

/// While a drag is held, what the other editor types and what a peer edits land beneath the
/// held layer edit by edit: neither editor is shown the content afresh.
#[test]
fn typing_and_a_peers_edit_during_a_held_drag_send_editors_no_snapshot() {
    let (mut sim, a, b) = dragging(2);
    sim.gesture(0, 1, Gesture::BeginTentative);
    sim.operate(0, 1, Checklist::set_text(a, "dragged"));
    sim.settle();
    for typed in ["t", "ty", "typ", "typed"] {
        sim.operate(0, 2, Checklist::set_text(b, typed));
        sim.settle();
    }
    let (_, added) = Checklist::add("peer");
    sim.operate(1, 1, added);
    sim.settle();
    for editor in [1, 2] {
        assert!(!snapshotted(&mut sim, 0, editor), "editor {editor}");
        assert_eq!(texts(sim.mirror(0, editor)), ["dragged", "typed", "peer"]);
    }
    assert_eq!(texts(&sim.content(1)), ["a", "typed", "peer"]);

    sim.gesture(0, 1, Gesture::Commit);
    sim.settle();
    for device in 0..2 {
        assert_eq!(texts(&sim.content(device)), ["dragged", "typed", "peer"]);
        assert_eq!(texts(sim.mirror(device, 2)), ["dragged", "typed", "peer"]);
    }
    assert!(!snapshotted(&mut sim, 0, 2));
}
