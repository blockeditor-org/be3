use super::*;

#[test]
fn a_followers_kept_edits_survive_the_owner_leaving_before_saving_them() {
    let mut sim = Sim::<ChecklistContent>::new(2, checklist_module);
    sim.open(0);
    sim.open(1);
    sim.attach(0, 1, true);
    sim.attach(1, 1, true);
    sim.deaf.insert(0);
    sim.operate(1, 1, add("follower's"));
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
    assert_eq!(texts(&sim.content(1)), ["replacement"]);
    assert_eq!(texts(sim.mirror(1, 1)), ["replacement"]);
    assert!(sim.kept(|content| texts(content).contains(&"follower's".to_owned())));
}
