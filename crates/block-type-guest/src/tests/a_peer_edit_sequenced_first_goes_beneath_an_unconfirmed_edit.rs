use super::*;

#[test]
fn a_peer_edit_sequenced_first_goes_beneath_an_unconfirmed_edit() {
    let mut sim = Sim::<ChecklistContent>::new(2, checklist_module);
    sim.open(0);
    sim.open(1);
    sim.attach(0, 1, true);
    sim.attach(1, 1, true);
    sim.take_received(1, 1);
    sim.operate(0, 1, add("first"));
    sim.operate(1, 1, add("second"));
    sim.settle();
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
        .expect("the edit the server ordered first lands beneath the unconfirmed one");
    assert!(!beneath.mine);
    assert_eq!(beneath.beneath.as_ref().map(Vec::len), Some(1));
    let expected = vec!["first".to_owned(), "second".to_owned()];
    for device in 0..2 {
        assert_eq!(texts(&sim.content(device)), expected);
        assert_eq!(texts(sim.mirror(device, 1)), expected);
    }
}
