use super::*;

#[test]
fn an_external_publish_merges_into_a_live_replica() {
    let mut sim = Sim::<ChecklistContent>::new(2, checklist_module);
    sim.open(0);
    sim.open(1);
    sim.attach(0, 1, true);
    sim.attach(1, 1, true);
    sim.operate(0, 1, add("unsealed"));
    sim.settle();
    let mut external = ChecklistContent::default();
    external.apply(&add("external"));
    sim.publish_elsewhere(&external);
    assert_eq!(sim.rejections, 0);
    assert_eq!(sim.publishes, 1);
    let mut merged = texts(&sim.head_content());
    merged.sort();
    assert_eq!(merged, ["external", "unsealed"]);
    let head = texts(&sim.head_content());
    for device in 0..2 {
        assert_eq!(texts(&sim.content(device)), head);
        assert_eq!(texts(sim.mirror(device, 1)), head);
    }
    let mut again = sim.head_content();
    again.apply(&add("again"));
    sim.publish_elsewhere(&again);
    assert_eq!(sim.publishes, 1);
    for device in 0..2 {
        assert_eq!(texts(&sim.content(device)), texts(&again));
        assert_eq!(texts(sim.mirror(device, 1)), texts(&again));
        assert_eq!(sim.devices[device].status.head, sim.head);
    }
}
