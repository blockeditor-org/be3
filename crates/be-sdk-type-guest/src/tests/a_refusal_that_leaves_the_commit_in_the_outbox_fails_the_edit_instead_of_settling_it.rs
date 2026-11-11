use super::*;

#[test]
fn a_refusal_that_leaves_the_commit_in_the_outbox_fails_the_edit_instead_of_settling_it() {
    let mut sim = Sim::<ChecklistContent>::new(1, checklist_module);
    sim.kept_publishes = true;
    sim.unheld_appends = true;
    sim.open(0);
    sim.attach(0, 1, true);
    sim.send_from_editor(0, 1, FromEditor::FollowLacking);
    sim.settle();
    sim.operate(0, 1, add("sent offline"));
    sim.settle();
    sim.advance(crate::QUIET_INTERVAL);
    assert_eq!(sim.lacking(0, 1), 1, "the commit is kept in the outbox");

    // The server refused the commit, which stays in the outbox to be tried again, so no refused
    // commit is named: a refusal is not mistaken for the outbox draining.
    sim.with_replica(0, |replica, host| replica.drained(host, true, Vec::new()));
    sim.settle();
    assert_eq!(sim.lacking(0, 1), 1, "the edit is still lacking");
    assert!(
        sim.editors[&(0, 1)]
            .lacking()
            .iter()
            .all(|lacked| lacked.failed),
        "and shows as not sent"
    );
}
