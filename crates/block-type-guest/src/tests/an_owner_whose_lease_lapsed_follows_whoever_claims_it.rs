use super::*;

#[test]
fn an_owner_whose_lease_lapsed_follows_whoever_claims_it() {
    let mut sim = Sim::<CounterContent>::new(2, counter_module);
    sim.lease = Lease::lasting(0);
    for device in 0..2 {
        sim.open(device);
        sim.attach(device, 1, true);
    }
    sim.operate(0, 1, Counter::add(1));
    sim.settle();
    assert_eq!(count(sim.mirror(1, 1)), 1);
    assert!(sim.devices[0].status.owner);

    sim.claim(1);
    sim.operate(0, 1, Counter::add(10));
    sim.operate(1, 1, Counter::add(100));
    sim.settle();
    assert!(
        !sim.devices[0].status.owner,
        "the old owner kept sequencing"
    );
    assert!(
        sim.devices[1].status.owner,
        "the lapsed lease was not taken"
    );
    assert_eq!(sim.publishes, 0);

    sim.advance(crate::SEAL_INTERVAL);
    assert_eq!(sim.publishes, 1, "only the new owner saves");
    assert_eq!(count(&sim.head_content()), 111);
    for device in 0..2 {
        assert_eq!(count(&sim.content(device)), 111);
        assert_eq!(count(sim.mirror(device, 1)), 111);
    }

    sim.operate(1, 1, Counter::add(1_000));
    sim.operate(0, 1, Counter::add(10_000));
    sim.settle();
    sim.advance(crate::SEAL_INTERVAL);
    assert_eq!(count(&sim.head_content()), 11_111);
    for device in 0..2 {
        assert_eq!(count(&sim.content(device)), 11_111);
        assert_eq!(sim.devices[device].status.head, sim.head);
    }
    assert!(sim.devices[0].status.clean && !sim.devices[0].status.owner);
}
