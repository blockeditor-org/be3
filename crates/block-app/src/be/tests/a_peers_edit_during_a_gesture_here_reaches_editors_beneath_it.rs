use super::*;

use be_client::Live;

#[test]
fn a_peers_edit_during_a_gesture_here_reaches_editors_beneath_it() {
    let harness = Harness::start();
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("a test runtime starts");
    let block = Uuid::new_v4();
    let mut owner = runtime.block_on(async {
        let outside = harness.outside_peer().await;
        outside
            .ensure::<CounterContent>(block, be_graph::BlockParent::Root)
            .await
            .expect("the counter is created");
        Live::<_, CounterContent>::join(std::sync::Arc::clone(&outside), block)
            .await
            .expect("the outside peer owns the session")
    });
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

    runtime.block_on(async {
        owner
            .edit(Counter::add(10))
            .await
            .expect("the owner adds before it reads this peer's adds");
        while owner.content().root().value() != 13 {
            owner.wait().await.expect("the session is still up");
        }
    });
    wait_until("took the owner's add beneath its own", |shared| {
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
        operations
            .iter()
            .any(|operation| operation.beneath.is_some()),
        "the owner's add goes beneath this peer's adds: {operations:?}"
    );
    assert_eq!(editor.mirror.pending(), 0);
}
