use super::*;

#[test]
fn a_followers_kept_edits_reach_the_owner_that_takes_over() {
    let mut sim = Sim::<ChecklistContent>::new(3, checklist_module);
    for device in 0..3 {
        sim.open(device);
        sim.attach(device, 1, true);
    }
    sim.deaf.insert(0);
    sim.operate(2, 1, add("follower's"));
    sim.settle();
    let mut replacement = ChecklistContent::default();
    replacement.apply(&add("replacement"));
    sim.send_from_editor(0, 1, FromEditor::Replace(replacement.encode()));
    sim.settle();
    sim.crash(0);
    sim.deaf.clear();
    sim.advance(crate::SEAL_INTERVAL);
    assert!(sim.devices[1].status.owner);
    assert_eq!(texts(&sim.head_content()), ["replacement"]);
    for device in 1..3 {
        assert_eq!(texts(&sim.content(device)), ["replacement"]);
        assert_eq!(texts(sim.mirror(device, 1)), ["replacement"]);
    }
    assert!(sim.kept(|content| texts(content).contains(&"follower's".to_owned())));
}
