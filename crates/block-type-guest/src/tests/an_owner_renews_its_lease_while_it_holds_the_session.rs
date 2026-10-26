use super::*;

#[test]
fn an_owner_renews_its_lease_while_it_holds_the_session() {
    let mut sim = Sim::<CounterContent>::new(2, counter_module);
    sim.open(0);
    sim.open(1);
    sim.attach(0, 1, true);
    assert!(sim.devices[0].status.owner);
    assert_eq!(
        sim.devices[0].wake, None,
        "an owner with nothing to do woke for a heartbeat"
    );
    let generation = sim.lease.generation();

    sim.stalled = true;
    sim.operate(0, 1, Counter::add(1));
    sim.settle();
    for _ in 0..6 {
        sim.advance(crate::HEARTBEAT_INTERVAL);
        assert!(
            !sim.lease.is_expired(sim.now as i64 + 1),
            "the owner let its lease lapse while its seal was waiting"
        );
    }
    assert_eq!(sim.lease.generation(), generation);
    assert_eq!(sim.lease.owner(), Some(sim.devices[0].client));
    assert_eq!(
        sim.lease.clean_at(),
        None,
        "a dirty owner reported itself clean"
    );
}
