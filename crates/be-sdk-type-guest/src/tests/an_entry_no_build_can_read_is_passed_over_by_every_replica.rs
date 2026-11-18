use super::*;

/// An entry of the order this build cannot read is passed over by every replica alike, so their
/// copies still agree: each says the session is incompatible, none diverges, and editing and
/// saving go on.
#[test]
fn an_entry_no_build_can_read_is_passed_over_by_every_replica() {
    let mut sim = Sim::<ChecklistContent>::new(2, checklist_module);
    sim.open(0);
    sim.open(1);
    sim.attach(0, 1, true);
    sim.attach(1, 1, true);
    sim.operate(0, 1, add("kept"));
    sim.settle();
    sim.submit_as(1, vec![0xff, 0xff, 0xff, 0xff]);
    for device in 0..2 {
        assert!(sim.devices[device].status.incompatible, "device {device}");
        assert!(!sim.devices[device].status.diverged, "device {device}");
    }

    sim.operate(1, 1, add("after"));
    sim.settle();
    for device in 0..2 {
        assert_eq!(
            texts(&sim.content(device)),
            ["kept", "after"],
            "device {device}"
        );
    }
    sim.advance(crate::QUIET_INTERVAL);
    assert_eq!(texts(&sim.head_content()), ["kept", "after"]);
}
