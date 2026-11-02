use super::*;

#[test]
fn a_rejected_seal_merges_the_newer_head_and_publishes_again() {
    let mut sim = Sim::<ChecklistContent>::new(2, checklist_module);
    sim.open(0);
    sim.open(1);
    sim.attach(1, 1, true);
    sim.attach(0, 1, true);
    sim.operate(0, 1, add("ours"));
    sim.settle();
    let mut theirs = ChecklistContent::default();
    theirs.apply(&add("theirs"));
    sim.quiet = true;
    sim.publish_elsewhere(&theirs);
    sim.quiet = false;
    sim.advance(crate::SEAL_INTERVAL);
    assert_eq!(sim.rejections, 1);
    let mut merged = texts(&sim.head_content());
    merged.sort();
    assert_eq!(merged, ["ours", "theirs"]);
    assert!(sim.conflicts.is_empty());
    let head = texts(&sim.head_content());
    for device in 0..2 {
        assert_eq!(texts(&sim.content(device)), head);
        assert_eq!(texts(sim.mirror(device, 1)), head);
        assert_eq!(sim.devices[device].status.head, sim.head);
    }
    assert!(sim.devices[0].status.clean);
}
