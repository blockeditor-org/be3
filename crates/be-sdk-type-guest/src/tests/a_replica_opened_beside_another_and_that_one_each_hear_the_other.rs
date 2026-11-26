use super::*;

/// A replica opens in a session another is in already: the one already there sees it join and
/// tells it what it shows (`SessionMessage::Shown`), and the one opening, which never sees the
/// other join, tells it alike as it starts. Each ends with the other in `heard`, though neither
/// shows anything.
#[test]
fn a_replica_opened_beside_another_and_that_one_each_hear_the_other() {
    let mut sim = Sim::<CounterContent>::new(2, counter_module);
    sim.open(0);
    sim.attach(0, 1, true);
    sim.open(1);
    sim.attach(1, 1, true);
    sim.settle();
    let (first, second) = (sim.devices[0].client, sim.devices[1].client);
    assert!(
        sim.editors[&(1, 1)].heard(first),
        "the replica opened second heard the one there before it"
    );
    assert!(
        sim.editors[&(0, 1)].heard(second),
        "the replica there first heard the one opened beside it"
    );
    assert!(sim.errors.is_empty(), "{:?}", sim.errors);
}
