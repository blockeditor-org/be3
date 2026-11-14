use super::*;

/// The owner's editor replaces the content while the server holds back what is published. The
/// replacement is a commit of its own, not sequenced, so until it lands the status does not
/// count it among the edits the head holds (`Status::sealed_taken`): the host keeps it to give
/// again. Once it lands, it does.
#[test]
fn a_replacement_on_its_way_is_not_counted_as_held_by_the_head() {
    let mut sim = Sim::<ChecklistContent>::new(1, checklist_module);
    sim.open(0);
    sim.attach(0, 1, true);
    sim.operate(0, 1, add("before"));
    sim.settle();
    sim.advance(crate::QUIET_INTERVAL);
    sim.settle();
    let status = sim.devices[0].status.clone();
    assert_eq!(status.sealed_taken, status.taken, "the head holds the edit");

    sim.stalled = true;
    let mut replacement = ChecklistContent::default();
    let (_, created) = Checklist::add("after");
    replacement.apply(&created);
    sim.send_from_editor(0, 1, FromEditor::Replace(replacement.encode()));
    sim.settle();
    let status = sim.devices[0].status.clone();
    assert!(
        status.sealed_taken < status.taken,
        "the replacement on its way is not held by the head: {} of {}",
        status.sealed_taken,
        status.taken
    );

    sim.unstall();
    sim.settle();
    let status = sim.devices[0].status.clone();
    assert_eq!(
        status.sealed_taken, status.taken,
        "once it lands, the head holds it"
    );
}
