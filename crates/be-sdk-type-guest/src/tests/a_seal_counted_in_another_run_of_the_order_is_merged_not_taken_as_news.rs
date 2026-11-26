use super::*;

/// A seal on the commit a replica is on, counted in another run of the order (the session
/// restarted since), says nothing of how far this run's numbers reach: the replica merges it as
/// a head, rather than taking it as holding what it numbered, which would drop its edit.
#[test]
fn a_seal_counted_in_another_run_of_the_order_is_merged_not_taken_as_news() {
    let mut sim = Sim::<CounterContent>::new(1, counter_module);
    sim.open(0);
    sim.attach(0, 1, true);
    sim.operate(0, 1, Counter::add(1));
    sim.settle();
    for _ in 0..3 {
        sim.advance(crate::QUIET_INTERVAL);
        sim.settle();
    }
    assert_eq!(count(&sim.head_content()), 1);
    let on = sim.head.expect("sealed");

    sim.stalled = true;
    sim.operate(0, 1, Counter::add(5));
    sim.settle();

    // Another run of the order on the same commit sealed this, through the same number.
    sim.quiet = true;
    let mut other = sim.head_content();
    other.apply(&Counter::add(100));
    let elsewhere = sim.publish_elsewhere(&other);
    let through = sim.journal.as_ref().map_or(0, |journal| {
        journal
            .entries
            .last()
            .map_or(journal.base.sequence, |entry| entry.sequence)
    });
    let stored = sim.commits.get_mut(&elsewhere).expect("stored");
    assert_eq!(stored.parents, [on]);
    stored.sealed_through = Some((through, 1));
    sim.quiet = false;
    sim.moved(elsewhere, None);
    sim.settle();
    sim.unstall();
    sim.settle();
    for _ in 0..3 {
        sim.advance(crate::QUIET_INTERVAL);
        sim.settle();
    }
    assert_eq!(count(&sim.content(0)), 106, "the edit was kept");
    assert_eq!(count(&sim.head_content()), 106);
    assert!(sim.errors.is_empty(), "{:?}", sim.errors);
}
