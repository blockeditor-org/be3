use std::time::Instant;

use be_block_content::{BlockContent, LiveEdit};
use be_sdk_type_guest::editor::{FromEditor, Gesture};
use be_type_example_counter::{Counter, CounterContent};

use super::*;

/// Sends the open counter `message` from `editor`.
fn send(replicas: &mut Replicas<Quiet>, block: Uuid, editor: EditorId, message: FromEditor) {
    replicas.relay_from_editor(block, editor, message.encode(), Instant::now());
}

fn add(by: i64) -> FromEditor {
    FromEditor::Operate(CounterContent::encode_operation(&Counter::add(by)))
}

/// The counter its replica shows now, without a held drag.
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

/// One editor holds a drag adding 10 while another adds 1, which the order holds and a seal
/// keeps: the host keeps the drag's messages to give again, where they arrived, though the
/// seal holds the edit after them. The module stops mid-drag; the replica opened again holds
/// the drag again, and its commit adds 10 once, through another restart too.
#[test]
fn a_module_restart_mid_drag_keeps_the_drag() {
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
        let (dragging, typing) = (7, 8);
        replicas.attach(block, dragging, true);
        replicas.attach(block, typing, true);
        send(
            &mut replicas,
            block,
            dragging,
            FromEditor::Gesture(Gesture::BeginTentative),
        );
        send(&mut replicas, block, dragging, add(10));
        send(&mut replicas, block, typing, add(1));
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
        let status = replicas.status(block).expect("the counter is open");
        assert_eq!(status.taken, 1, "the held messages are no local edits yet");
        assert_eq!(status.sealed_taken, 1);
        assert_eq!(
            replicas.replayed_edits(block),
            0,
            "the seal holds every local edit; the held drag stays, out of the count"
        );
        assert_eq!(count(&peer, &shared, &mut replicas, block).await, 1);

        replicas.modules.restart_type(CounterContent::CONTENT_TYPE);
        replicas.pump((&peer, &shared)).await;
        while replicas.joining() {
            step(&peer, &shared, &mut replicas).await;
        }
        replicas.pump((&peer, &shared)).await;
        send(
            &mut replicas,
            block,
            dragging,
            FromEditor::Gesture(Gesture::Commit),
        );
        while replicas
            .status(block)
            .is_none_or(|status| status.taken < 1 || !status.journaled)
        {
            step(&peer, &shared, &mut replicas).await;
        }
        replicas.pump((&peer, &shared)).await;
        assert_eq!(
            count(&peer, &shared, &mut replicas, block).await,
            11,
            "the drag the replica opened again rebuilt commits"
        );

        replicas.modules.restart_type(CounterContent::CONTENT_TYPE);
        replicas.pump((&peer, &shared)).await;
        while replicas.joining() {
            step(&peer, &shared, &mut replicas).await;
        }
        replicas.pump((&peer, &shared)).await;
        assert_eq!(
            count(&peer, &shared, &mut replicas, block).await,
            11,
            "the committed drag is given once"
        );
    });
}
