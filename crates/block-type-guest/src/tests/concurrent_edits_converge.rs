use super::*;

#[test]
fn concurrent_edits_converge() {
    let mut sim = Sim::<ChecklistContent>::new(3, checklist_module);
    for device in 0..3 {
        sim.open(device);
        sim.attach(device, 1, true);
    }
    sim.operate(0, 1, add("a"));
    sim.operate(1, 1, add("b"));
    sim.operate(2, 1, add("c"));
    sim.operate(1, 1, add("d"));
    sim.settle();
    let owner = texts(&sim.content(0));
    assert_eq!(owner.len(), 4);
    for device in 0..3 {
        assert_eq!(texts(&sim.content(device)), owner);
        assert_eq!(texts(sim.mirror(device, 1)), owner);
    }
    sim.advance(crate::SEAL_INTERVAL);
    assert_eq!(texts(&sim.head_content()), owner);
    for device in 0..3 {
        assert_eq!(sim.devices[device].status.head, sim.head);
        assert!(sim.devices[device].status.clean);
    }
}
