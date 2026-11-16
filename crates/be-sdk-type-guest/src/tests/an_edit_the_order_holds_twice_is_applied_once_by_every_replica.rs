use super::*;

/// An edit the order took twice (given again under a new counter after its first submission
/// reached the order unseen, as after a crash) is applied once by every replica alike: the
/// second carries a `through` its origin's entries already reached.
#[test]
fn an_edit_the_order_holds_twice_is_applied_once_by_every_replica() {
    let mut sim = Sim::<CounterContent>::new(2, counter_module);
    sim.open(0);
    sim.open(1);
    sim.attach(0, 1, true);
    sim.attach(1, 1, true);
    sim.operate(0, 1, Counter::add(1));
    sim.settle();
    let first = sim
        .journal
        .as_ref()
        .and_then(|journal| journal.entries.last().cloned())
        .expect("the order holds the edit");
    sim.number(
        0,
        be_sdk_type_api::Submission {
            origin: first.origin,
            counter: first.counter + 1_000,
            base: sim.order.sequence,
            barrier: false,
            payload: first.payload.clone(),
        },
    );
    sim.settle();
    for device in 0..2 {
        assert_eq!(count(&sim.content(device)), 1, "device {device}");
    }

    sim.operate(1, 1, Counter::add(10));
    sim.settle();
    for device in 0..2 {
        assert_eq!(count(&sim.content(device)), 11, "device {device}");
    }
    sim.advance(crate::QUIET_INTERVAL);
    assert_eq!(count(&sim.head_content()), 11);
}
