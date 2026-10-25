use super::*;

#[test]
fn a_message_no_build_can_read_is_passed_over_by_every_replica() {
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
    sim.act(
        1,
        Command::Submit {
            replica: REPLICA,
            id: be_session::OpId {
                origin: 9,
                counter: 1,
            },
            base: 1,
            barrier: false,
            payload: vec![0xff, 0xff, 0xff, 0xff],
        },
    );
    sim.settle();
    for device in 0..2 {
        assert!(sim.devices[device].status.incompatible);
        assert!(
            !sim.devices[device].status.diverged,
            "every replica passed over the same entry, so every copy is still right"
        );
    }

    sim.operate(1, 1, add("after"));
    sim.settle();
    for device in 0..2 {
        assert_eq!(texts(&sim.content(device)), ["kept", "after"]);
    }
    sim.advance(crate::SEAL_INTERVAL);
    assert_eq!(sim.publishes, 1);
    assert_eq!(texts(&sim.head_content()), ["kept", "after"]);
}
