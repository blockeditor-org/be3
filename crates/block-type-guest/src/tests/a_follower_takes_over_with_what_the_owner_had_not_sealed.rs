use super::*;

#[test]
fn a_follower_takes_over_with_what_the_owner_had_not_sealed() {
    let mut sim = Sim::<CounterContent>::new(3, counter_module);
    for device in 0..3 {
        sim.open(device);
        sim.attach(device, 1, true);
    }
    sim.operate(0, 1, Counter::add(1));
    sim.operate(1, 1, Counter::add(2));
    sim.settle();
    assert_eq!(sim.publishes, 0);
    sim.crash(0);
    assert!(sim.devices[1].status.owner);
    assert!(sim.devices[1].status.owes_seal);
    sim.operate(2, 1, Counter::add(4));
    sim.settle();
    sim.advance(crate::SEAL_INTERVAL);
    assert_eq!(sim.publishes, 1);
    assert_eq!(count(&sim.head_content()), 7);
    for device in 1..3 {
        assert_eq!(count(&sim.content(device)), 7);
        assert_eq!(count(sim.mirror(device, 1)), 7);
        assert_eq!(sim.devices[device].status.head, sim.head);
    }
}
