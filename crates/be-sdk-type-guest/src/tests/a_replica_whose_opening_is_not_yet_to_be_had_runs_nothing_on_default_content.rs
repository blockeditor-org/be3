use super::*;

#[test]
fn a_replica_whose_opening_is_not_yet_to_be_had_runs_nothing_on_default_content() {
    let mut sim = Sim::<CounterContent>::new(2, counter_module);
    sim.open(0);
    sim.attach(0, 1, true);
    sim.operate(0, 1, Counter::add(3));
    sim.settle();
    sim.close(0);
    let head = sim.head;
    assert_eq!(count(&sim.head_content()), 3);

    sim.not_yet_loads = 1;
    sim.open(1);
    assert_eq!(sim.not_yet_loads, 0, "the opening read the head");
    let (publishes, numbered, parts_put) = (sim.publishes, sim.numbered, sim.parts_put);

    let ticket = sim.with_replica(1, |replica, host| replica.content(host));
    let outcome = sim.answered(1, ticket.op());
    assert_eq!(ticket.decode(outcome).ok(), Some(None), "no content is shown");
    assert!(!sim.change(1, Change::Undo));
    let ticket = sim.with_replica(1, |replica, host| replica.seal(host, true));
    let outcome = sim.answered(1, ticket.op());
    assert!(ticket.decode(outcome).is_ok());

    assert_eq!(sim.publishes, publishes, "nothing was sealed");
    assert_eq!(sim.numbered, numbered, "nothing was ordered");
    assert_eq!(sim.parts_put, parts_put, "nothing was put");
    assert_eq!(sim.head, head);

    sim.close(1);
    assert!(sim.devices[1].released, "closing released the replica");
    assert_eq!(sim.publishes, publishes);
    assert_eq!(sim.head, head);
}
