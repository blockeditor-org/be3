use super::*;

#[test]
fn a_follower_replaces_the_content_and_the_owner_takes_it() {
    let mut sim = Sim::<ChecklistContent>::new(2, checklist_module);
    sim.open(0);
    sim.open(1);
    sim.attach(0, 1, true);
    sim.attach(1, 1, true);
    sim.operate(0, 1, add("before"));
    sim.settle();
    let mut replacement = ChecklistContent::default();
    let (item, created) = Checklist::add("after");
    replacement.apply(&created);
    sim.send_from_editor(1, 1, FromEditor::Replace(replacement.encode()));
    sim.settle();
    assert!(!sim.devices[1].status.owner);
    assert_eq!(texts(&sim.head_content()), ["after"]);
    assert!(sim.kept(|content| texts(content).contains(&"before".to_owned())));
    for device in 0..2 {
        assert_eq!(texts(&sim.content(device)), ["after"]);
        assert_eq!(texts(sim.mirror(device, 1)), ["after"]);
    }
    sim.operate(0, 1, Checklist::set_text(item, "renamed"));
    sim.settle();
    assert_eq!(texts(sim.mirror(1, 1)), ["renamed"]);
}
