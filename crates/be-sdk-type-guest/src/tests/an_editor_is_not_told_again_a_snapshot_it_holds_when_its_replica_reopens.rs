use super::*;

/// A replica opened while its block's creation waited in the outbox is opened again once the
/// creation reaches the server and the replica joins its session, and the host attaches its
/// editors again. An editor told nothing since its snapshot holds what the reopened replica
/// would tell it, so it is not told it twice; one that asks (`Resend`, as a restarted app does)
/// is told it all the same.
#[test]
fn an_editor_is_not_told_again_a_snapshot_it_holds_when_its_replica_reopens() {
    let mut sim = Sim::<ChecklistContent>::new(1, checklist_module);
    sim.open(0);
    sim.attach(0, 1, true);
    let received = sim.take_received(0, 1);
    assert!(
        matches!(received.as_slice(), [ToEditor::Snapshot(_)]),
        "an editor attached is told its snapshot: {received:?}"
    );

    sim.reopen(0);
    let received = sim.take_received(0, 1);
    assert!(
        received.is_empty(),
        "the reopened replica tells the editor nothing it holds: {received:?}"
    );

    sim.send_from_editor(0, 1, FromEditor::Resend);
    sim.settle();
    let received = sim.take_received(0, 1);
    assert!(
        matches!(received.as_slice(), [ToEditor::Snapshot(_)]),
        "an editor that asks is told its snapshot again: {received:?}"
    );
}
