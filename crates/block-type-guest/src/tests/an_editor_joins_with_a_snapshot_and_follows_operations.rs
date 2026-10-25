use super::*;

#[test]
fn an_editor_joins_with_a_snapshot_and_follows_operations() {
    let mut sim = Sim::<ChecklistContent>::new(2, checklist_module);
    sim.open(0);
    sim.open(1);
    sim.attach(1, 1, true);
    sim.operate(1, 1, add("first"));
    sim.settle();
    sim.attach(1, 2, true);
    let received = sim.take_received(1, 2);
    let [ToEditor::Snapshot(snapshot)] = received.as_slice() else {
        panic!("a joining editor is sent a snapshot first");
    };
    assert_eq!(snapshot.unsettled.len(), 1);
    assert_eq!(snapshot.applied, 0);
    assert_eq!(texts(sim.mirror(1, 2)), ["first"]);
    sim.take_received(1, 1);
    sim.operate(1, 2, add("second"));
    sim.settle();
    let mine = |messages: Vec<ToEditor>| -> Vec<bool> {
        messages
            .into_iter()
            .flat_map(|message| match message {
                ToEditor::Operations(operations) => operations,
                _ => Vec::new(),
            })
            .map(|operation| operation.mine)
            .collect()
    };
    assert_eq!(mine(sim.take_received(1, 1)), [false]);
    assert_eq!(mine(sim.take_received(1, 2)), [true]);
    sim.attach(0, 1, false);
    sim.operate(0, 1, add("ignored"));
    sim.settle();
    assert_eq!(texts(&sim.content(0)), ["first", "second"]);
    sim.send_from_editor(1, 2, FromEditor::Resend);
    sim.settle();
    let received = sim.take_received(1, 2);
    let [ToEditor::Snapshot(snapshot)] = received.as_slice() else {
        panic!("an editor that asks is sent a snapshot");
    };
    assert_eq!(snapshot.applied, 1);
    for (device, editor) in [(1, 1), (1, 2)] {
        assert_eq!(texts(sim.mirror(device, editor)), ["first", "second"]);
        assert_eq!(sim.editors[&(device, editor)].pending(), 0);
    }
    sim.detach(1, 2);
    sim.operate(0, 1, add("unseen"));
    sim.operate(1, 1, add("third"));
    sim.settle();
    assert!(sim.take_received(1, 2).is_empty());
}
