use super::*;

#[test]
fn an_owner_that_loses_the_session_with_an_append_in_flight_keeps_its_edit_lacking() {
    let mut sim = Sim::<CounterContent>::new(2, counter_module);
    sim.lease = Lease::lasting(0);
    for device in 0..2 {
        sim.open(device);
        sim.attach(device, 1, true);
    }
    sim.follow_lacking(0, 1);
    assert!(sim.devices[0].status.owner);

    sim.unheld_appends = true;
    sim.operate(0, 1, Counter::add(1));
    sim.settle();
    sim.claim(1);
    sim.settle();
    sim.unheld_appends = false;
    assert!(!sim.devices[0].status.owner, "the other device took the session");
    assert_eq!(
        sim.lacking(0, 1),
        1,
        "an edit whose append the server never took is not shown as held when its owner \
         becomes a follower"
    );
    assert_eq!(
        count(sim.mirror(1, 1)),
        0,
        "the new owner never had it: it is lost, so it keeps showing as sending"
    );
}
