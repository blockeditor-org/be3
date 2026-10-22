use super::*;

use be_block::{Landed, Settling};
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
    wait_for_count(block, 0);
    let sent = content(block).expect("the counter is held").revision;
    for _ in 0..3 {
        add(block, 1);
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

    let held = content(block).expect("the counter is held");
    let Update::Operations(operations) = held.since(TEST_ORIGIN, Some(sent)) else {
        panic!("an editor that had the counter is sent operations, not a snapshot");
    };
    assert!(
        operations
            .iter()
            .any(|operation| operation.beneath.is_some()),
        "the owner's add goes beneath this peer's adds: {operations:?}"
    );
    let mut editor = Settling::new(CounterContent::default(), Vec::new());
    for operation in &operations {
        let decode =
            |bytes: &[u8]| CounterContent::decode_operation(bytes).expect("the operation decodes");
        let landed = match &operation.beneath {
            Some(local) => Landed::Beneath {
                operation: decode(&operation.operation),
                unsettled: operation.unsettled,
                local: local.iter().map(|bytes| decode(bytes)).collect(),
            },
            None => Landed::On {
                operation: decode(&operation.operation),
                unsettled: operation.unsettled,
            },
        };
        assert!(
            editor.land(landed),
            "each operation lands where the editor is"
        );
    }
    assert_eq!(editor.visible().root().value(), 13);
}
