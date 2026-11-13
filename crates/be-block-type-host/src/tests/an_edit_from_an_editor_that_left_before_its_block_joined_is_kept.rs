use std::time::Instant;

use be_block::{BlockContent, LiveEdit};
use be_sdk_type_guest::editor::FromEditor;
use be_type_text::{TextBlock, TextContent};

use super::*;

/// What the open text holds now, as its replica shows it.
async fn text(
    peer: &Arc<Peer<MemoryStore>>,
    shared: &Arc<Mutex<Quiet>>,
    replicas: &mut Replicas<Quiet>,
    block: Uuid,
) -> String {
    let bytes = replicas
        .content((peer, shared), block)
        .await
        .expect("the text is open");
    TextBlock::text(&TextContent::decode(&bytes).expect("the text decodes"))
}

/// A block opened while its creation waits to be sent journals in a session of its own, and
/// joins the server's once the drain sends that: its replica opens again on the head. An
/// editor that typed and left meanwhile is not attached to it any more, yet what it typed, kept
/// to be given to the reopened replica, is taken as it was.
#[test]
fn an_edit_from_an_editor_that_left_before_its_block_joined_is_kept() {
    with_test_types(|peer, shared, mut replicas| async move {
        let block = Uuid::new_v4();
        peer.keep_creation(
            block,
            TextContent::CONTENT_TYPE,
            BlockParent::Root,
            &be_block::BlockMetadata::default(),
            false,
        )
        .await
        .expect("the block is kept here");
        replicas
            .open((&peer, &shared), block, TextContent::CONTENT_TYPE, true)
            .await;
        replicas.pump((&peer, &shared)).await;
        assert!(!replicas.joining());

        let editor = 7;
        replicas.attach(block, editor, true);
        replicas.pump((&peer, &shared)).await;
        let content = TextContent::decode(
            &replicas
                .content((&peer, &shared), block)
                .await
                .expect("the text is open"),
        )
        .expect("the text decodes");
        let typed =
            TextBlock::insert(&content, 1, 0, b"socks").expect("there is somewhere to type");
        replicas.relay_from_editor(
            block,
            editor,
            FromEditor::Operate(TextContent::encode_operation(&typed)).encode(),
            Instant::now(),
        );
        // It leaves before the replica has said what it journaled.
        replicas.detach(block, editor);
        let given = |shared: &Arc<Mutex<Quiet>>| {
            let held = shared.lock().unwrap();
            held.to_editors.iter().filter(|to| **to == editor).count()
        };
        let before = given(&shared);

        peer.drain(|_| false).await.expect("the creation is sent");
        replicas.created((&peer, &shared));
        assert!(replicas.joining(), "once sent, the block joins its session");
        while replicas.joining() {
            replicas.woken().await;
            replicas.pump((&peer, &shared)).await;
        }
        replicas.pump((&peer, &shared)).await;
        assert_eq!(text(&peer, &shared, &mut replicas, block).await, "socks");
        // Attached for its edits alone, it is given nothing: no one would take it.
        assert_eq!(
            given(&shared),
            before,
            "the editor that left was given frames after it left"
        );
    });
}
