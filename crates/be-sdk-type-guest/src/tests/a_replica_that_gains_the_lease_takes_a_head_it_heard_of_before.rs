use super::*;

/// A replica told of a new head while another holds the lease leaves the head to that holder.
/// Should the lease come to it before the holder took the head (the holder never heard of it, or
/// there was none, as for a replica reopened alone), it takes the head itself once it holds the
/// lease, rather than staying on what it loaded.
#[test]
fn a_replica_that_gains_the_lease_takes_a_head_it_heard_of_before() {
    let mut sim = Sim::<CounterContent>::new(2, counter_module);
    for device in 0..2 {
        sim.open(device);
        sim.attach(device, 1, true);
    }
    assert!(sim.devices[0].status.holder);

    // The head moves on elsewhere, and only the replica without the lease hears of it.
    sim.quiet = true;
    let mut elsewhere = CounterContent::default();
    elsewhere.apply(&Counter::add(5));
    let head = sim.publish_elsewhere(&elsewhere);
    sim.quiet = false;
    sim.with_replica(1, |replica, host| replica.head_moved(host, head));
    sim.settle();
    assert_eq!(count(&sim.content(1)), 0, "a replica without the lease leaves the head");

    sim.claim(1);
    sim.settle();
    assert!(sim.devices[1].status.holder);
    assert_eq!(count(&sim.content(1)), 5, "the new holder took the head it heard of");
}
