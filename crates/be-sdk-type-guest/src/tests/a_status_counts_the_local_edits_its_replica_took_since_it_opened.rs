use super::*;

/// A status says how many local edits (an editor's messages and the host's changes) the
/// replica took since it opened, in the order they came: the host drops what it keeps to give a
/// reopened replica only once a clean status counts it, so an edit sent after the module made
/// that status is kept. One opened again counts from none.
#[test]
fn a_status_counts_the_local_edits_its_replica_took_since_it_opened() {
    let mut sim = Sim::<CounterContent>::new(1, counter_module);
    sim.open(0);
    sim.attach(0, 1, true);
    sim.settle();
    assert_eq!(sim.devices[0].status.taken, 0, "attaching is no edit");
    sim.operate(0, 1, Counter::add(2));
    sim.operate(0, 1, Counter::add(3));
    sim.settle();
    assert_eq!(sim.devices[0].status.taken, 2);
    sim.change(0, Change::Undo);
    sim.settle();
    assert_eq!(sim.devices[0].status.taken, 3);
    sim.reopen(0);
    sim.settle();
    assert_eq!(
        sim.devices[0].status.taken, 0,
        "opened again, it counts from none"
    );
}
