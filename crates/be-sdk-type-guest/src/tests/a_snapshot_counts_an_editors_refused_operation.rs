use super::*;

/// A replica counts every operation an attached editor sends, taken or refused, as the host
/// counts what it relays, and sends an editor whose operation it refused a snapshot at once:
/// the operation never lands as the editor's, and the editor lets go of it rather than taking a
/// later one landing as its own for it.
#[test]
fn a_snapshot_counts_an_editors_refused_operation() {
    let mut sim = Sim::<ChecklistContent>::new(2, checklist_module);
    sim.open(0);
    sim.attach(0, 1, false);
    let message = sim
        .editors
        .get_mut(&(0, 1))
        .and_then(|mirror| mirror.operate(add("refused")))
        .expect("an editor operates once it has content");
    sim.take_received(0, 1);
    sim.send_from_editor(0, 1, message);
    sim.settle();
    assert!(
        texts(&sim.content(0)).is_empty(),
        "a read-only editor's edit is dropped"
    );
    let received = sim.take_received(0, 1);
    let [ToEditor::Snapshot(snapshot)] = received.as_slice() else {
        panic!("an editor whose edit was refused is sent a snapshot: {received:?}");
    };
    assert_eq!(snapshot.applied, 1, "the refused operation is counted");
    assert_eq!(sim.editors[&(0, 1)].pending(), 0);
    assert!(texts(sim.mirror(0, 1)).is_empty());
}
