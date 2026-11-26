use std::time::Instant;

use be_block_content::{BlockContent, LiveEdit};
use be_sdk_type_guest::editor::FromEditor;
use be_type_example_counter::{Counter, CounterContent};

use super::*;

/// Adds `by` to the open counter as its editor.
fn add(replicas: &mut Replicas<Quiet>, block: Uuid, editor: EditorId, by: i64) {
    replicas.relay_from_editor(
        block,
        editor,
        FromEditor::Operate {
            gesture: None,
            operation: CounterContent::encode_operation(&Counter::add(by)),
        }
        .encode(),
        Instant::now(),
    );
}

/// The counter its replica shows now.
async fn count(
    peer: &Arc<Peer<MemoryStore>>,
    shared: &Arc<Mutex<Quiet>>,
    replicas: &mut Replicas<Quiet>,
    block: Uuid,
) -> i64 {
    let bytes = replicas
        .content((peer, shared), block)
        .await
        .expect("the counter is open");
    CounterContent::decode(&bytes)
        .expect("the counter decodes")
        .root()
        .value()
}

/// An editor adds 1, which the order holds and a seal keeps, and then 2, which the order holds
/// past that seal: the head holds the first and not the second, and the replica says so
/// (`Status::sealed_taken`), so the host keeps the second alone to give again. Its module stops,
/// and the replica opened again reads 3, not 4: an addition given twice counts twice.
#[test]
fn an_edit_ordered_after_a_seal_is_given_once_to_a_restarted_replica() {
    with_test_types(|peer, shared, mut replicas| async move {
        let block = peer
            .create::<CounterContent>(BlockParent::Root)
            .await
            .expect("the counter is made");
        replicas
            .open((&peer, &shared), block, CounterContent::CONTENT_TYPE, true)
            .await
            .expect("the block opens");
        replicas.pump((&peer, &shared)).await;
        let editor = 7;
        replicas.attach(block, editor, true);
        add(&mut replicas, block, editor, 1);
        while replicas
            .status(block)
            .is_none_or(|status| status.taken < 1 || !status.journaled)
        {
            step(&peer, &shared, &mut replicas).await;
        }
        let before = replicas.status(block).and_then(|status| status.head);
        replicas.seal_later(block, true);
        while replicas
            .status(block)
            .is_none_or(|status| status.head == before)
        {
            step(&peer, &shared, &mut replicas).await;
        }

        add(&mut replicas, block, editor, 2);
        while replicas
            .status(block)
            .is_none_or(|status| status.taken < 2 || !status.journaled)
        {
            step(&peer, &shared, &mut replicas).await;
        }
        let status = replicas.status(block).expect("the counter is open");
        assert_eq!(
            status.sealed_taken, 1,
            "the seal holds the first edit alone"
        );
        assert_eq!(
            replicas.replayed_edits(block),
            1,
            "the host keeps the second edit alone to give again"
        );
        assert_eq!(count(&peer, &shared, &mut replicas, block).await, 3);

        replicas.modules.restart_type(CounterContent::CONTENT_TYPE);
        replicas.pump((&peer, &shared)).await;
        while replicas.joining() {
            step(&peer, &shared, &mut replicas).await;
        }
        replicas.pump((&peer, &shared)).await;
        assert_eq!(
            count(&peer, &shared, &mut replicas, block).await,
            3,
            "each edit reaches the replica opened again once"
        );
    });
}
