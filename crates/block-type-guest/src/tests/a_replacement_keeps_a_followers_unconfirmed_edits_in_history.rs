use super::*;

#[test]
fn a_replacement_keeps_a_followers_unconfirmed_edits_in_history() {
    let mut sim = Sim::<ChecklistContent>::new(2, checklist_module);
    sim.open(0);
    sim.open(1);
    sim.attach(0, 1, true);
    sim.attach(1, 1, true);
    sim.operate(0, 1, add("owner's"));
    sim.settle();
    sim.operate(1, 1, add("follower's"));
    let mut replacement = ChecklistContent::default();
    replacement.apply(&add("replacement"));
    sim.send_from_editor(0, 1, FromEditor::Replace(replacement.encode()));
    sim.settle();
    assert!(sim.devices[0].status.owner);
    assert_eq!(texts(&sim.head_content()), ["replacement"]);
    for device in 0..2 {
        assert_eq!(texts(&sim.content(device)), ["replacement"]);
        assert_eq!(texts(sim.mirror(device, 1)), ["replacement"]);
    }
    assert!(sim.kept(|content| texts(content).contains(&"owner's".to_owned())));
    assert!(sim.kept(|content| texts(content).contains(&"follower's".to_owned())));
    sim.operate(1, 1, add("afterwards"));
    sim.settle();
    sim.advance(crate::SEAL_INTERVAL);
    assert_eq!(texts(&sim.head_content()), ["replacement", "afterwards"]);
    assert_eq!(texts(sim.mirror(0, 1)), ["replacement", "afterwards"]);
}
