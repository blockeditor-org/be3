use super::*;

/// A peer's edit the order numbers while this replica's own is on its way goes beneath it: the
/// editor is told so, and every replica ends with the order's content.
#[test]
fn a_peer_edit_numbered_first_goes_beneath_an_unconfirmed_edit() {
    let mut sim = Sim::<ChecklistContent>::new(2, checklist_module);
    sim.open(0);
    sim.open(1);
    sim.attach(0, 1, true);
    sim.attach(1, 1, true);
    sim.take_received(1, 1);
    sim.stalled_submits = true;
    sim.operate(1, 1, add("mine"));
    sim.settle();
    sim.stalled_submits = false;
    sim.operate(0, 1, add("theirs"));
    sim.settle();
    sim.unstall();
    let operations: Vec<_> = sim
        .take_received(1, 1)
        .into_iter()
        .flat_map(|message| match message {
            ToEditor::Operations(operations) => operations,
            _ => Vec::new(),
        })
        .collect();
    assert!(operations[0].mine);
    let beneath = operations
        .iter()
        .find(|operation| operation.beneath.is_some())
        .expect("the peer's edit lands beneath this replica's");
    assert!(!beneath.mine);
    assert_eq!(beneath.beneath.as_ref().map(Vec::len), Some(1));
    let expected = vec!["theirs".to_owned(), "mine".to_owned()];
    for device in 0..2 {
        assert_eq!(texts(&sim.content(device)), expected);
        assert_eq!(texts(sim.mirror(device, 1)), expected);
    }
}
