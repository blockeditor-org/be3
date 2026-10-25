use super::*;

#[test]
fn a_peers_edit_reaches_editors_here_as_an_operation() {
    let harness = Harness::start();
    let block = Uuid::new_v4();
    let mut owner = Outside::counter(&harness, block);
    assert!(owner.is_owner());

    harness.connect();
    open(block, CounterContent::CONTENT_TYPE);
    let mut editor = Editor::<CounterContent>::attach(block);
    editor.wait("was sent the counter", |counter| {
        counter.root().value() == 0
    });
    editor.take_received();
    for _ in 0..3 {
        editor.operate(Counter::add(1));
    }
    wait_for_count(block, 3);

    owner.add(10);
    owner.until("took this peer's adds", |seen| seen.count == Some(13));
    wait_until("took the owner's add", |shared| {
        counted(shared, block) == Some(13) && shared.unsealed == 0
    });
    editor.wait("followed the owner's add", |counter| {
        counter.root().value() == 13
    });

    let operations: Vec<_> = editor
        .take_received()
        .into_iter()
        .flat_map(|message| match message {
            block_type_guest::editor::ToEditor::Operations(operations) => operations,
            block_type_guest::editor::ToEditor::Snapshot(_) => {
                panic!("an editor that had the counter is sent operations, not a snapshot")
            }
            _ => Vec::new(),
        })
        .collect();
    assert!(
        operations.iter().any(|operation| !operation.mine),
        "the peer's add reached the editor as an operation: {operations:?}"
    );
    assert_eq!(editor.mirror.pending(), 0);
}
