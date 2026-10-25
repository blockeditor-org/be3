use super::*;

#[test]
fn closing_seals_and_releases() {
    let mut sim = Sim::<CounterContent>::new(2, counter_module);
    sim.open(0);
    sim.open(1);
    sim.attach(0, 1, true);
    sim.operate(0, 1, Counter::add(3));
    sim.settle();
    sim.close(0);
    assert_eq!(sim.publishes, 1);
    assert_eq!(count(&sim.head_content()), 3);
    assert!(sim.devices[0].module.is_none());
    assert!(sim.devices[1].status.owner);
    assert_eq!(sim.devices[1].status.head, sim.head);
}
