use super::*;

/// A replica opened again drops what answers the one before it (a later epoch), yet the host
/// answers the calls that one made all the same: a read of the log, a part put and an append
/// an editor asked for, still unanswered as it reopened, are each answered to the editor by
/// the new replica, and none is made again, so the append lands once.
#[test]
fn an_editors_calls_are_answered_once_though_its_replica_reopens_before_the_answers() {
    let mut sim = Sim::<Logbook>::new(1, logbook_module);
    sim.open(0);
    sim.attach(0, 1, true);
    sim.settle();
    sim.take_received(0, 1);

    // The editor asks; the host's answers wait while the replica opens again.
    let mirror = sim.editors.get_mut(&(0, 1)).unwrap();
    let (appended, append) = mirror.append(b"one".to_vec(), be_block_content::Along::default());
    let (put, put_part) = mirror.put_part(b"a part".to_vec());
    let (read, read_log) = mirror.read_log(0, 10);
    for message in [append, put_part, read_log] {
        sim.send_from_editor(0, 1, message);
    }
    sim.reopen(0);

    let received = sim.take_received(0, 1);
    let appends: Vec<_> = received
        .iter()
        .filter(|message| matches!(message, ToEditor::Appended { token, result: Ok(_) } if *token == appended))
        .collect();
    let puts: Vec<_> = received
        .iter()
        .filter(|message| matches!(message, ToEditor::PartPut { token, result: Ok(_) } if *token == put))
        .collect();
    let reads: Vec<_> = received
        .iter()
        .filter(|message| matches!(message, ToEditor::LogRead { token, result: Ok(_) } if *token == read))
        .collect();
    assert_eq!(
        (appends.len(), puts.len(), reads.len()),
        (1, 1, 1),
        "the append, the put and the read are each answered once: {received:?}"
    );
    assert_eq!(sim.log.len(), 1, "the append landed once");
    assert_eq!(sim.parts_put, 1, "the part was put once");
}
