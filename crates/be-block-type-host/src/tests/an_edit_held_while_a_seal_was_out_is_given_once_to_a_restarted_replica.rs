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
        FromEditor::Operate(CounterContent::encode_operation(&Counter::add(by))).encode(),
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

/// An editor adds 1 and then, while the seal of that is out, 2, which the replica holds back
/// and sequences once the seal lands: its head then holds the first and not the second, and it
/// says so (`Status::sealed_taken`), so the host keeps the second alone to give again, though
/// the replica is not clean. Its module stops, and the replica opened again on that head reads
/// 3, not 4: an addition given twice counts twice.
#[test]
fn an_edit_held_while_a_seal_was_out_is_given_once_to_a_restarted_replica() {
    with_test_types(|peer, shared, mut replicas| async move {
        let block = peer
            .create::<CounterContent>(BlockParent::Root)
            .await
            .expect("the counter is made");
        replicas
            .open((&peer, &shared), block, CounterContent::CONTENT_TYPE, true)
            .await;
        replicas.pump((&peer, &shared)).await;
        let editor = 7;
        replicas.attach(block, editor, true);
        add(&mut replicas, block, editor, 1);
        replicas.pump((&peer, &shared)).await;
        let before = replicas.status(block).and_then(|status| status.head);

        replicas.seal_later(block, true);
        add(&mut replicas, block, editor, 2);
        while replicas.status(block).is_none_or(|status| {
            status.head == before || status.taken < 2 || status.sealed_taken < 1
        }) {
            replicas.woken().await;
            replicas.pump((&peer, &shared)).await;
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
        assert_eq!(
            count(&peer, &shared, &mut replicas, block).await,
            3,
            "each edit reaches the replica opened again once"
        );
    });
}
