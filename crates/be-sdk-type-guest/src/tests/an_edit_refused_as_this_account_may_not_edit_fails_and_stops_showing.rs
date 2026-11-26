use super::*;

/// An edit the server refuses because this account may only view the block never reaches the
/// order: the replica stops showing it and tells its editor it failed, as one the order passed
/// over, rather than showing it as sending for ever; what is typed after goes, and is refused,
/// in turn, and the replica settles.
#[test]
fn an_edit_refused_as_this_account_may_not_edit_fails_and_stops_showing() {
    let mut sim = Sim::<ChecklistContent>::new(2, checklist_module);
    sim.open(0);
    sim.attach(0, 1, true);
    sim.operate(0, 1, add("made by an editor"));
    sim.settle();

    sim.devices[1].may_edit = false;
    sim.open(1);
    sim.attach(1, 1, true);
    sim.follow_lacking(1, 1);
    sim.operate(1, 1, add("refused"));
    sim.operate(1, 1, add("refused too"));
    sim.settle();

    assert_eq!(texts(&sim.content(1)), ["made by an editor"]);
    assert_eq!(texts(sim.mirror(1, 1)), ["made by an editor"]);
    assert_eq!(sim.lacking(1, 1), 2, "each shows until it is let go");
    assert!(
        sim.editors[&(1, 1)]
            .lacking()
            .iter()
            .all(|lacked| lacked.failed),
        "as not sent"
    );
    assert!(sim.devices[1].status.clean, "nothing waits to be sent");
    assert!(!sim.errors.is_empty(), "the refusal is told");
    assert_eq!(texts(&sim.content(0)), ["made by an editor"]);
}
