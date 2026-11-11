use super::*;

#[test]
fn a_followers_stash_kept_offline_by_the_new_owner_keeps_lacking_until_the_outbox_drains() {
    let mut sim = Sim::<ChecklistContent>::new(2, checklist_module);
    sim.open(0);
    sim.open(1);
    sim.attach(0, 1, true);
    sim.attach(1, 1, true);
    sim.follow_lacking(1, 1);
    sim.deaf.insert(0);
    sim.operate(1, 1, add("follower's"));
    sim.settle();
    let mut replacement = ChecklistContent::default();
    replacement.apply(&add("replacement"));
    sim.send_from_editor(0, 1, FromEditor::Replace(replacement.encode()));
    sim.settle();
    sim.kept_publishes = true;
    sim.crash(0);
    sim.deaf.clear();
    sim.advance(crate::QUIET_INTERVAL);
    assert!(sim.devices[1].status.owner);
    assert_eq!(
        sim.lacking(1, 1),
        1,
        "the stash sits in this device's outbox: the server lacks it"
    );

    sim.with_replica(1, |replica, host| replica.drained(host, false));
    sim.settle();
    assert_eq!(sim.lacking(1, 1), 0, "the outbox drained");
}
