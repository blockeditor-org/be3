use super::*;

#[test]
fn an_edit_resubmitted_after_a_reconnect_lands_once() {
    let mut sim = Sim::<CounterContent>::new(2, counter_module);
    for device in 0..2 {
        sim.open(device);
        sim.attach(device, 1, true);
    }

    sim.stalled.insert(1);
    sim.operate(1, 1, Counter::add(1));
    sim.settle();
    sim.reconnect(1);
    sim.unstall(1);
    for device in 0..2 {
        assert_eq!(
            count(&sim.content(device)),
            1,
            "the edit and its resubmission both landed"
        );
    }
    assert!(sim.devices[1].status.clean);

    sim.deaf.insert(1);
    sim.operate(1, 1, Counter::add(10));
    sim.settle();
    sim.advance(crate::SEAL_INTERVAL);
    assert_eq!(count(&sim.head_content()), 11);
    assert_eq!(sim.held(), 0);
    sim.deaf.remove(&1);
    sim.reconnect(1);
    for device in 0..2 {
        assert_eq!(count(&sim.content(device)), 11);
        assert_eq!(count(sim.mirror(device, 1)), 11);
    }
    assert!(
        sim.devices[1].status.clean,
        "an edit the server sealed was still waiting to be confirmed"
    );
}
