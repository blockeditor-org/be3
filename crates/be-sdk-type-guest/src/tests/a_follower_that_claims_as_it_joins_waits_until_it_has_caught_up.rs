use super::*;

#[test]
fn a_follower_that_claims_as_it_joins_waits_until_it_has_caught_up() {
    let mut sim = Sim::<CounterContent>::new(3, counter_module);
    sim.lease = Lease::lasting(0);
    sim.open(0);
    sim.attach(0, 1, true);
    for _ in 0..3 {
        sim.operate(0, 1, Counter::add(1));
    }
    sim.settle();

    sim.deaf.insert(1);
    sim.open(1);
    sim.attach(1, 1, true);
    sim.open(2);
    sim.attach(2, 1, true);
    sim.claim(1);
    sim.settle();
    assert_eq!(
        sim.lease.owner(),
        Some(sim.devices[0].client),
        "claimed before hearing what the owner had not saved"
    );

    sim.deaf.remove(&1);
    sim.resynchronize(1);
    assert_eq!(count(sim.mirror(1, 1)), 3);
    sim.claim(1);
    sim.settle();
    assert!(
        sim.devices[1].status.owner,
        "the laptop never took the session"
    );

    sim.operate(1, 1, Counter::add(10));
    sim.operate(2, 1, Counter::add(100));
    sim.settle();
    for device in 0..3 {
        assert_eq!(count(&sim.content(device)), 113);
    }
    assert!(!sim.devices[0].status.owner);
    assert!(sim.devices[2].status.clean);
    sim.advance(crate::SEAL_INTERVAL);
    assert_eq!(count(&sim.head_content()), 113);
}
