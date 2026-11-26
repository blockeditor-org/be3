use super::*;
use be_block_session::SessionMessage;

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

/// What `device` told its peers of its tentative layers since the last call: each `Held` as
/// (from, to, operations, commits_through).
pub(super) fn told_held(
    sim: &mut Sim<ChecklistContent>,
    device: usize,
) -> Vec<(u64, u64, Vec<Edit>, Option<u64>)> {
    std::mem::take(&mut sim.sent)
        .into_iter()
        .filter(|(from, _)| *from == device)
        .filter_map(|(_, bytes)| match crate::wire::decode(&bytes)? {
            SessionMessage::Held {
                from,
                to,
                operations,
                commits_through,
                ..
            } => Some((
                from,
                to,
                operations
                    .iter()
                    .map(|operation| ChecklistContent::decode_operation(operation).unwrap())
                    .collect(),
                commits_through,
            )),
            _ => None,
        })
        .collect()
}

/// The most local edits of `device`'s origin the order holds.
pub(super) fn journaled_through(sim: &Sim<ChecklistContent>, device: usize) -> u64 {
    let origin = sim.devices[device].origin;
    sim.journal
        .as_ref()
        .map(|journal| {
            journal
                .entries
                .iter()
                .filter(|entry| entry.origin == origin)
                .filter_map(|entry| Some(Submitted::decode(&entry.payload)?.through))
                .max()
                .unwrap_or(0)
        })
        .unwrap_or(0)
}

/// What `editor` on `device` was told since the last call: how many snapshots, and each
/// landing.
pub(super) fn landings(
    sim: &mut Sim<ChecklistContent>,
    device: usize,
    editor: EditorId,
) -> (usize, Vec<crate::editor::EditorOperation>) {
    let received = sim.take_received(device, editor);
    let snapshots = received
        .iter()
        .filter(|message| matches!(message, ToEditor::Snapshot(_)))
        .count();
    let landed = received
        .into_iter()
        .flat_map(|message| match message {
            ToEditor::Operations(operations) => operations,
            _ => Vec::new(),
        })
        .collect();
    (snapshots, landed)
}

/// Hands `to` a `Held` as `from` would tell it: layer `layer` whole, its edits `operations`.
pub(super) fn relay_held(
    sim: &mut Sim<ChecklistContent>,
    to: usize,
    from: usize,
    layer: u64,
    operations: &[Edit],
    commits_through: Option<u64>,
) {
    let held = SessionMessage::Held {
        layer,
        origin: sim.devices[from].origin,
        from: 0,
        to: operations.len() as u64,
        operations: operations
            .iter()
            .map(|operation| ChecklistContent::encode_operation(operation).into())
            .collect(),
        commits_through,
    };
    let client = sim.devices[from].client;
    sim.relay_as(to, client, crate::wire::encode(&held));
    sim.settle();
}

/// How many times `device` asked its peers for a layer whole since `sim.sent` was cleared.
pub(super) fn asked_again(sim: &Sim<ChecklistContent>, device: usize) -> usize {
    sim.sent
        .iter()
        .filter(|(from, bytes)| {
            *from == device && crate::wire::decode(bytes) == Some(SessionMessage::HeldAgain)
        })
        .count()
}
