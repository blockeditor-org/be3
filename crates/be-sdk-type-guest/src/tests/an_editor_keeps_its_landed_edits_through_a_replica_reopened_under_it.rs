use super::*;

/// A host that rejoins a block's session (its creation reached the server, or it reconnected)
/// reopens the replica on what was sealed and gives it again the editor's operations it kept
/// since. The editor, still open, was already told those landed. The reopened replica sent it a
/// snapshot as it attached, before they came again, counting none of them: the editor rebuilt
/// its view on a snapshot that lacked them, and its edits made on top of them no longer applied.
/// Its snapshot now waits for `resent`, after them, and counts them.
#[test]
fn an_editor_keeps_its_landed_edits_through_a_replica_reopened_under_it() {
    let mut sim = Sim::<ChecklistContent>::new(2, checklist_module);
    sim.open(0);
    sim.attach(0, 1, true);
    let mut sent = Vec::new();
    for text in ["first", "second"] {
        let message = sim
            .editors
            .get_mut(&(0, 1))
            .and_then(|mirror| mirror.operate(add(text)))
            .expect("an editor operates once it has content");
        sim.send_from_editor(0, 1, message.clone());
        sent.push(message);
    }
    sim.settle();
    assert_eq!(sim.editors[&(0, 1)].acknowledged(), 2);
    sim.take_received(0, 1);

    // Nothing was sealed or journaled, so the replica opens on the head holding none of the
    // editor's operations, and the host gives it both again.
    let opening = Opening {
        content_type: ChecklistContent::CONTENT_TYPE,
        ..opening(sim.devices[0].client, sim.head, sim.lease.state().clone())
    };
    sim.with_replica(0, |replica, host| replica.reopen(host, opening));
    sim.with_replica(0, |replica, host| replica.attach(host, 1, true, 0));
    for message in sent {
        sim.send_from_editor(0, 1, message);
    }
    sim.settle();
    assert!(
        sim.take_received(0, 1).is_empty(),
        "the editor is sent nothing until what was kept of its edits is in"
    );
    sim.with_replica(0, |replica, host| replica.resent(host, 1));
    sim.settle();
    let received = sim.take_received(0, 1);
    let [ToEditor::Snapshot(snapshot)] = received.as_slice() else {
        panic!("the editor is sent its snapshot once its edits were given again: {received:?}");
    };
    assert_eq!(
        snapshot.applied, 2,
        "the snapshot counts both edits given again"
    );
    assert_eq!(texts(sim.mirror(0, 1)), ["first", "second"]);

    sim.operate(0, 1, add("third"));
    sim.settle();
    assert_eq!(texts(&sim.content(0)), ["first", "second", "third"]);
    assert_eq!(texts(sim.mirror(0, 1)), ["first", "second", "third"]);
    assert_eq!(sim.editors[&(0, 1)].pending(), 0);
}
