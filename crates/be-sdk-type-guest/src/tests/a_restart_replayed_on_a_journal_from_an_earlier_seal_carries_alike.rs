use super::*;

/// The server loses the rebase of the seal a restart goes on, so the journal stays on the seal
/// before it and holds the entries before the restart's seal, the restart, and the edits after
/// it. A replica opening on it before the next seal reads the commit the restart names and
/// replays the restart as the participants applied it: its content is theirs.
#[test]
fn a_restart_replayed_on_a_journal_from_an_earlier_seal_carries_alike() {
    let mut sim = Sim::<ProseContent>::new(3, prose_module);
    sim.open(0);
    sim.open(1);
    sim.attach(0, 1, true);
    sim.attach(1, 1, true);
    sim.type_at(0, 1, 0, "start\n");
    sim.advance(crate::QUIET_INTERVAL);
    assert_eq!(sim.devices[0].status.restarts, 0, "the first seal does not restart");
    let first = sim.head;
    // From here a seal restarts, even with the peer in the session.
    crate::replica::RESTART_LIMIT.with(|limit| limit.set(8));
    for round in 0..20 {
        sim.type_at(0, 1, 0, &format!("scratch {round} "));
        let typed = sim.typed(0, 1).find("start").expect("the text is there");
        sim.erase(0, 1, 0..typed);
    }
    // What the next seal commits.
    sim.type_at(0, 1, 0, "x ");
    sim.settle();

    // The holder's seal is out while the peer types; the server takes it as the head but keeps
    // the journal on the first seal.
    sim.lose_rebases = true;
    sim.stalled = true;
    sim.advance(crate::QUIET_INTERVAL);
    sim.type_at(1, 1, 0, "b ");
    sim.settle();
    let held = std::mem::take(&mut sim.held);
    sim.stalled = false;
    for (device, call) in held {
        sim.act(device, call);
    }
    sim.stalled = true;
    sim.settle();
    assert_ne!(sim.head, first, "the seal moved the head");
    assert_eq!(
        sim.journal.as_ref().map(|journal| journal.base.commit),
        Some(first),
        "the journal stays on the first seal"
    );

    sim.type_at(0, 1, 0, "a ");
    sim.type_at(1, 1, 0, "c ");
    sim.settle();
    assert!(sim.devices[0].status.restarts >= 1, "the restart landed");
    assert_eq!(prose(&sim.content(1)), prose(&sim.content(0)));

    sim.open(2);
    sim.attach(2, 1, true);
    sim.settle();
    assert_eq!(
        prose(&sim.content(2)),
        prose(&sim.content(0)),
        "the replica opened on the journal holds what the participants hold"
    );
    assert_eq!(sim.devices[2].status.restarts, sim.devices[0].status.restarts);
    // And it goes on with them: what it types reaches both.
    sim.type_at(2, 1, 0, "d ");
    sim.settle();
    assert!(sim.typed(0, 1).starts_with("d "), "{:?}", sim.typed(0, 1));
    assert_eq!(prose(&sim.content(1)), prose(&sim.content(2)));
}
