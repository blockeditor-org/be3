use super::*;

#[test]
fn edits_are_sealed_once_they_rest_for_the_seal_interval() {
    let mut sim = Sim::<CounterContent>::new(2, counter_module);
    sim.open(0);
    sim.open(1);
    sim.attach(0, 1, true);
    sim.operate(0, 1, Counter::add(2));
    sim.settle();
    assert!(sim.devices[0].status.owes_seal);
    assert_eq!(sim.devices[0].wake, Some(sim.now + crate::SEAL_INTERVAL));
    sim.advance(crate::SEAL_INTERVAL - 1);
    assert_eq!(sim.publishes, 0);
    sim.advance(1);
    assert_eq!(sim.publishes, 1);
    assert_eq!(count(&sim.head_content()), 2);
    assert!(!sim.devices[0].status.owes_seal);
    assert_eq!(sim.devices[1].status.head, sim.head);
    assert_eq!(sim.lease.clean_at(), sim.head);
    sim.advance(10 * crate::SEAL_INTERVAL);
    assert_eq!(sim.publishes, 1);
}
