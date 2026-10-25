use super::*;

#[test]
fn a_follower_edit_goes_beneath_a_peer_edit_the_owner_ordered_first() {
    let mut sim = Sim::<ChecklistContent>::new(2, checklist_module);
    sim.open(0);
    sim.open(1);
    sim.attach(0, 1, true);
    sim.attach(1, 1, true);
    sim.take_received(1, 1);
    sim.operate(1, 1, add("follower"));
    sim.operate(0, 1, add("owner"));
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
        .expect("the owner's edit lands beneath the follower's");
    assert!(!beneath.mine);
    assert_eq!(beneath.beneath.as_ref().map(Vec::len), Some(1));
    let expected = vec!["owner".to_owned(), "follower".to_owned()];
    for device in 0..2 {
        assert_eq!(texts(&sim.content(device)), expected);
        assert_eq!(texts(sim.mirror(device, 1)), expected);
    }
}
