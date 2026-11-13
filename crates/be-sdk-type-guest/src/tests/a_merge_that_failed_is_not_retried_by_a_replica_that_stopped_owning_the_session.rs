use super::*;

#[test]
fn a_merge_that_failed_is_not_retried_by_a_replica_that_stopped_owning_the_session() {
    // The owner's merge with a head that moved cannot read its part, so a retry waits on a timer.
    let mut sim = Sim::<ChecklistContent>::new(2, merging_module);
    sim.lease = Lease::lasting(0);
    for device in 0..2 {
        sim.open(device);
        sim.attach(device, 1, true);
    }
    sim.operate(0, 1, add("unsealed"));
    sim.settle();
    let part = sim
        .vault
        .write(
            SEALING,
            Uuid::nil(),
            be_block_content::Part::from(&b"a closed segment"[..]),
        )
        .unwrap()
        .root
        .unwrap();
    PART.with(|held| held.set(Some(part)));
    sim.fail_reads = 1_000;
    let mut external = ChecklistContent::default();
    external.apply(&add("external"));
    sim.publish_elsewhere(&external);
    assert_eq!(sim.errors.len(), 1, "the failure is reported once");
    let published = sim.publishes;

    // Another device takes the session over; the old owner's timer passes and it asks for nothing.
    sim.claim(1);
    sim.settle();
    assert!(!sim.devices[0].status.owner, "the old owner gave way");
    let resumed = sim.resumed[0];
    sim.advance(120_000);
    assert_eq!(
        sim.resumed[0], resumed,
        "a replica that is not the owner does not reconcile on its old timer"
    );
    assert_eq!(sim.publishes, published, "and publishes nothing");
}
