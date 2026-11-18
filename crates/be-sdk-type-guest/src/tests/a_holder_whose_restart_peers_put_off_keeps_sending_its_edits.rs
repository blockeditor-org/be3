use super::*;

/// A holder with a peer in its session, whose state is past where a restart is due even with
/// others, seals while the peer types: the seal falls short of what the order numbered, and the
/// shortfall is the peer's. The holder does not hold its own edits back for the restart, which
/// would leave them unsent for as long as the peer writes: what it types next reaches the peer
/// while its next seal is still out.
#[test]
fn a_holder_whose_restart_peers_put_off_keeps_sending_its_edits() {
    crate::replica::RESTART_LIMIT.with(|limit| limit.set(8));
    let mut sim = Sim::<ProseContent>::new(2, prose_module);
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
    assert_eq!(sim.devices[0].status.restarts, 0, "the restart is still put off");
}
