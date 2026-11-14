use super::*;

/// The host tells a replica which head a drain published. An edit kept in a later seal, which
/// the outbox still holds, stays sending, whenever the drain's answer arrives.
#[test]
fn a_drain_of_an_earlier_seal_leaves_an_edit_kept_after_it_sending() {
    let mut sim = Sim::<ChecklistContent>::new(1, checklist_module);
    sim.kept_publishes = true;
    sim.unheld_appends = true;
    sim.open(0);
    sim.attach(0, 1, true);
    sim.send_from_editor(0, 1, FromEditor::FollowLacking);
    sim.settle();
    sim.operate(0, 1, add("first"));
    sim.settle();
    sim.advance(crate::QUIET_INTERVAL);
    let first = sim.head.expect("the first seal was kept");
    sim.operate(0, 1, add("second"));
    sim.settle();
    sim.advance(crate::QUIET_INTERVAL);
    assert_ne!(
        sim.head,
        Some(first),
        "the second edit is in a seal of its own"
    );
    assert_eq!(sim.lacking(0, 1), 2);

    sim.with_replica(0, |replica, host| replica.drained(host, false, vec![first]));
    sim.settle();
    assert_eq!(
        sim.lacking(0, 1),
        1,
        "the edit in the seal the drain did not publish is still sending"
    );

    let second = sim.head.expect("the second seal was kept");
    sim.with_replica(0, |replica, host| {
        replica.drained(host, false, vec![second])
    });
    sim.settle();
    assert_eq!(sim.lacking(0, 1), 0);
}
