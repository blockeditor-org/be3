use super::*;

#[test]
fn a_promoted_followers_handed_stash_stops_lacking_once_it_keeps_it_itself() {
    let mut sim = Sim::<ChecklistContent>::new(2, checklist_module);
    sim.open(0);
    sim.open(1);
    sim.attach(0, 1, true);
    sim.attach(1, 1, true);
    sim.follow_lacking(1, 1);
    sim.deaf.insert(0);
    sim.operate(1, 1, add("follower's"));
    sim.settle();
    assert_eq!(sim.lacking(1, 1), 1);
    let mut replacement = ChecklistContent::default();
    replacement.apply(&add("replacement"));
    sim.send_from_editor(0, 1, FromEditor::Replace(replacement.encode()));
    sim.settle();
    sim.crash(0);
    sim.deaf.clear();
    sim.advance(crate::QUIET_INTERVAL);
    assert!(sim.devices[1].status.owner);
    assert!(sim.kept(|content| texts(content).contains(&"follower's".to_owned())));
    assert_eq!(
        sim.lacking(1, 1),
        0,
        "the follower took the session over and kept its handed stash itself: the server holds it"
    );
}
