use super::*;

/// A holder with a peer in its session, whose state is past where a restart is due even with
/// others, seals while the peer types: the seal falls short of what the order numbered. The
/// restart goes on it all the same, carrying the peer's edit over it, and the holder's edits go
/// on: what it types next reaches the peer, and nothing either typed is lost. A replica opening on
/// the journal before the next seal replays the restart alike.
#[test]
fn a_holder_whose_seal_a_peer_outpaces_restarts_and_loses_nothing() {
    crate::replica::RESTART_LIMIT.with(|limit| limit.set(8));
    let mut sim = Sim::<ProseContent>::new(3, prose_module);
    sim.open(0);
    sim.open(1);
    sim.attach(0, 1, true);
    sim.attach(1, 1, true);
    sim.type_at(0, 1, 0, "start\n");
    for round in 0..20 {
        sim.type_at(0, 1, 0, &format!("scratch {round} "));
        let typed = sim.typed(0, 1).find("start").expect("the text is there");
        sim.erase(0, 1, 0..typed);
    }
    sim.settle();

    // The holder's seal is out while the peer types.
    sim.stalled = true;
    sim.advance(crate::QUIET_INTERVAL);
    sim.type_at(1, 1, 0, "b ");
    sim.settle();
    // Its answer comes back short; the next seal stays out.
    let held = std::mem::take(&mut sim.held);
    sim.stalled = false;
    for (device, call) in held {
        sim.act(device, call);
    }
    sim.stalled = true;
    sim.settle();

    sim.type_at(0, 1, 0, "a ");
    sim.settle();
    assert!(
        sim.typed(1, 1).starts_with("a b "),
        "the holder's edit reached the peer: {:?}",
        sim.typed(1, 1)
    );
    assert!(sim.devices[0].status.restarts >= 1, "the restart did not wait");
    assert!(
        sim.typed(0, 1).starts_with("a b "),
        "and the holder holds both: {:?}",
        sim.typed(0, 1)
    );

    // A replica opening on the journal before the next seal (it stays out) replays the restart
    // as the participants applied it.
    sim.open(2);
    sim.attach(2, 1, true);
    sim.settle();
    assert_eq!(prose(&sim.content(2)), prose(&sim.content(0)));
    assert_eq!(prose(&sim.content(1)), prose(&sim.content(0)));
}
