use super::*;

/// A seal can fail where it is made, as one offline does when its block's key is not on the
/// device: the work stays journaled and the replica tries again on a timer, owing no seal
/// meanwhile, or the host, which seals what is owed on every hurried frame, would fail it on
/// every frame.
#[test]
fn a_seal_that_failed_is_tried_again_on_a_timer_and_owes_nothing_meanwhile() {
    let mut sim = Sim::<CounterContent>::new(1, counter_module);
    sim.open(0);
    sim.attach(0, 1, true);
    sim.operate(0, 1, Counter::add(2));
    sim.settle();
    assert!(sim.devices[0].status.owes_seal);
    sim.fail_publishes = usize::MAX;
    sim.advance(crate::QUIET_INTERVAL);
    assert_eq!(sim.failed_publishes, 1, "the due seal was made and failed");
    assert!(
        !sim.devices[0].status.owes_seal,
        "a replica whose seal just failed owes one at once"
    );
    sim.operate(0, 1, Counter::add(1));
    sim.settle();
    assert!(
        sim.devices[0].status.journaled,
        "an edit made while the seal waits is journaled at once, as the seal would have"
    );
    for _ in 0..60 {
        sim.advance(1_000);
    }
    assert!(
        sim.failed_publishes <= 6,
        "the failed seal is tried again on a timer, not every tick: {} tries in a minute",
        sim.failed_publishes
    );

    sim.fail_publishes = 0;
    sim.advance(2 * 60_000);
    assert_eq!(sim.publishes, 1, "once a seal can be made, it is");
    assert_eq!(count(&sim.head_content()), 3);
    assert!(!sim.devices[0].status.owes_seal);
}
