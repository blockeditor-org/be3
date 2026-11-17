use std::time::Instant;

use be_block_content::{BlockContent, LiveEdit};
use be_sdk_type_guest::editor::{FromEditor, Gesture};
use be_type_example_counter::{Counter, CounterContent};

use super::*;

fn send(replicas: &mut Replicas<Quiet>, block: Uuid, editor: EditorId, message: FromEditor) {
    replicas.relay_from_editor(block, editor, message.encode(), Instant::now());
}

/// An edit adding `by`, made in the editor's gesture `gesture` or outside any.
fn add(by: i64, gesture: Option<u64>) -> FromEditor {
    FromEditor::Operate {
        gesture,
        operation: CounterContent::encode_operation(&Counter::add(by)),
    }
}

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

/// Offline, one editor holds a drag adding 10 while another adds 1, and the host keeps its
/// edits on the device and stops. Started again, the replica holds the 1 and nothing of the
/// drag: its messages were never kept, so even a commit from an editor of that name commits
/// nothing.
#[test]
fn a_host_crash_with_a_held_drag_saves_nothing() {
    let clock = be_block_client::Clock::manual();
    let modules_clock = clock.clone();
    with_test_types_on(clock.clone(), |peer, shared, replicas| async move {
        let block = peer
            .create::<CounterContent>(BlockParent::Root)
            .await
            .expect("the counter is made");
        peer.save(block, &CounterContent::default(), None)
            .await
            .expect("the counter is published");
        peer.detach();
        let mut replicas = replicas;
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
            FromEditor::Gesture { id: 0, gesture: Gesture::BeginTentative },
        );
        send(&mut replicas, block, dragging, add(10, Some(0)));
        send(&mut replicas, block, typing, add(1, None));
        replicas.pump((&peer, &shared)).await;
        replicas.modules.tick_due();
        replicas.pump((&peer, &shared)).await;
        replicas.keep_all((&peer, &shared)).await;
        assert_eq!(count(&peer, &shared, &mut replicas, block).await, 1);
        drop(replicas);

        let mut replicas = Replicas::new(test_modules(modules_clock.clone()).await);
        replicas
            .open((&peer, &shared), block, CounterContent::CONTENT_TYPE, true)
            .await
            .expect("the block opens");
        replicas.pump((&peer, &shared)).await;
        replicas.attach(block, dragging, true);
        send(
            &mut replicas,
            block,
            dragging,
            FromEditor::Gesture { id: 0, gesture: Gesture::Commit },
        );
        replicas.pump((&peer, &shared)).await;
        assert_eq!(
            count(&peer, &shared, &mut replicas, block).await,
            1,
            "nothing of the held drag survives the crash"
        );
    });
}
