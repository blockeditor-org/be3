use super::*;

/// The lease follows from the session's membership, so its holder has nothing to renew: one
/// with nothing to seal sets no timer.
#[test]
fn a_holder_with_nothing_to_do_sets_no_timer() {
    let mut sim = Sim::<CounterContent>::new(2, counter_module);
    sim.open(0);
    sim.open(1);
    sim.attach(0, 1, true);
    assert!(sim.devices[0].status.holder);
    assert_eq!(
        sim.devices[0].wake, None,
        "an owner with nothing to do set a timer"
    );
}
