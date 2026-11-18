use super::*;

/// A replica whose head does not decode in this build tells each editor it failed, with why,
/// rather than sending a snapshot of empty content as though the block were empty.
#[test]
fn a_replica_whose_content_does_not_decode_tells_its_editors_it_failed() {
    let mut sim = Sim::<ChecklistContent>::new(1, checklist_module);
    let head = sim.write(vec![0xff; 7], Vec::new());
    sim.head = Some(head);
    sim.open(0);
    sim.attach(0, 1, true);
    let received = sim.take_received(0, 1);
    assert!(
        matches!(received.as_slice(), [ToEditor::Failed(_)]),
        "the editor is told the content will not open: {received:?}"
    );
    assert!(sim.editors[&(0, 1)].failure().is_some());
    assert!(!sim.editors[&(0, 1)].is_loaded());
}
