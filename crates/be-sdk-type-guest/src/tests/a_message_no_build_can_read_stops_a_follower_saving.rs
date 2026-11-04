use super::*;

#[test]
fn a_message_no_build_can_read_stops_a_follower_saving() {
    let mut sim = Sim::<ChecklistContent>::new(2, checklist_module);
    sim.open(0);
    sim.open(1);
    sim.attach(0, 1, true);
    sim.attach(1, 1, true);
    sim.operate(0, 1, add("kept"));
    sim.settle();
    for device in 0..2 {
        sim.relay(device, 9, vec![0xff, 0xff, 0xff, 0xff]);
    }
    sim.settle();
    for device in 0..2 {
        assert!(sim.devices[device].status.incompatible);
    }
    assert!(sim.devices[1].status.diverged);
    assert!(
        !sim.devices[0].status.diverged,
        "the owner's own copy is still right"
    );

    sim.operate(1, 1, add("refused"));
    sim.settle();
    assert_eq!(texts(&sim.content(0)), ["kept"]);
    assert!(sim.errors.iter().any(|(device, _)| *device == 1));

    sim.advance(crate::SEAL_INTERVAL);
    assert_eq!(sim.publishes, 1, "the owner still saves its edit");
    assert_eq!(texts(&sim.head_content()), ["kept"]);
}
