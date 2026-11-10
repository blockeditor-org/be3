use super::*;

/// An owner whose lease passes to another replica while the put of its paste is unanswered
/// follows that replica: the paste goes to the new owner by its part and lands once.
#[test]
fn an_owner_demoted_while_its_put_is_pending_sends_the_edit_to_the_new_owner() {
    let mut sim = Sim::<ProseContent>::new(2, prose_module);
    sim.lease = Lease::lasting(0);
    sim.publish_elsewhere(&prose_of("start\n"));
    for device in 0..2 {
        sim.open(device);
        sim.attach(device, 1, true);
    }
    assert!(sim.devices[0].status.owner);
    sim.stalled = true;
    let pasted = "a line of pasted text\n".repeat(60_000);
    sim.type_at(0, 1, 6, &pasted);
    sim.settle();
    assert_eq!(sim.held.len(), 1, "the put is unanswered");

    sim.claim(1);
    sim.type_at(1, 1, 0, "> ");
    sim.settle();
    assert!(sim.devices[1].status.owner, "the lease was taken");
    assert!(!sim.devices[0].status.owner, "the old owner follows");

    sim.unstall();
    sim.settle();
    let at_start = |text: &str| text.starts_with("> start\n") && text.len() == 8 + pasted.len();
    for device in 0..2 {
        assert!(at_start(&prose(&sim.content(device))), "device {device}");
        assert!(at_start(&sim.typed(device, 1)), "device {device}");
    }
    sim.advance(crate::QUIET_INTERVAL);
    assert!(prose(&sim.head_content()) == format!("> start\n{pasted}"));
    assert!(sim.errors.is_empty(), "{:?}", sim.errors);
}
