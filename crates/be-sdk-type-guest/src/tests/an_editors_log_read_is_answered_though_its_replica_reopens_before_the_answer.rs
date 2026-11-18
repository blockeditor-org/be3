use super::*;

/// A replica opened again drops what answers the one before it (a later epoch), so an editor's
/// read of the log the host had not answered as it reopened is asked for again by the new
/// replica, and the editor is answered all the same.
#[test]
fn an_editors_log_read_is_answered_though_its_replica_reopens_before_the_answer() {
    let mut sim = Sim::<Logbook>::new(1, logbook_module);
    sim.open(0);
    sim.attach(0, 1, true);
    let mirror = sim.editors.get_mut(&(0, 1)).unwrap();
    let (appended, message) = mirror.append(b"one".to_vec(), be_block_content::Along::default());
    sim.send_from_editor(0, 1, message);
    sim.settle();
    sim.take_received(0, 1);

    // The host is asked for the log; its answer waits while the replica opens again.
    let (token, message) = sim.editors.get_mut(&(0, 1)).unwrap().read_log(0, 10);
    assert_ne!(token, appended);
    sim.send_from_editor(0, 1, message);
    sim.reopen(0);

    let read: Vec<_> = sim
        .take_received(0, 1)
        .into_iter()
        .filter_map(|message| match message {
            ToEditor::LogRead { token: answered, result } if answered == token => Some(result),
            _ => None,
        })
        .collect();
    assert!(
        matches!(read.as_slice(), [Ok(page)] if page.items.len() == 1),
        "the editor's read is answered once, with the log: {read:?}"
    );
}
