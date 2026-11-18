use super::*;

/// A live checklist on `devices` devices, each with editors 1 and 2 attached, holding the items
/// `a` and `b`, settled: the ids of the items.
pub(super) fn dragging(devices: usize) -> (Sim<ChecklistContent>, ObjectId, ObjectId) {
    let mut sim = Sim::<ChecklistContent>::new(devices, checklist_module);
    for device in 0..devices {
        sim.open(device);
        sim.attach(device, 1, true);
        sim.attach(device, 2, true);
    }
    let (a, created) = Checklist::add("a");
    sim.operate(0, 1, created);
    let (b, created) = Checklist::add("b");
    sim.operate(0, 1, created);
    sim.settle();
    for device in 0..devices {
        sim.take_received(device, 1);
        sim.take_received(device, 2);
    }
    (sim, a, b)
}

/// Whether the order holds `operation`.
pub(super) fn journaled(sim: &Sim<ChecklistContent>, operation: &Edit) -> bool {
    sim.journal_operations().contains(operation)
}

/// Whether `editor` on `device` was told the replica ended its gesture, cancelled.
pub(super) fn told_ended(sim: &mut Sim<ChecklistContent>, device: usize, editor: EditorId) -> bool {
    sim.take_received(device, editor).iter().any(|message| {
        matches!(
            message,
            ToEditor::GestureEnded {
                cancelled: true,
                ..
            }
        )
    })
}
