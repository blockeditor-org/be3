use std::time::Instant;

use be_block_content::{BlockContent, LiveEdit};
use be_sdk_type_guest::editor::FromEditor;
use be_type_text::{TextBlock, TextContent};

use super::*;

/// A block whose creation the drain sent while its replica owes a seal is asked for that seal
/// once: `created` runs on every pass of the host's loop, and asking again while the seal is on
/// its way would wake the module, and the loop, every pass.
#[test]
fn a_block_sent_while_its_replica_owes_a_seal_is_asked_for_it_once() {
    with_test_types(|peer, shared, mut replicas| async move {
        let block = Uuid::new_v4();
        peer.keep_creation(
            block,
            TextContent::CONTENT_TYPE,
            BlockParent::Root,
            &be_block_content::BlockMetadata::default(),
            false,
        )
        .await
        .expect("the block is kept here");
        replicas
            .open((&peer, &shared), block, TextContent::CONTENT_TYPE, true)
            .await;
        replicas.pump((&peer, &shared)).await;

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
        replicas.pump((&peer, &shared)).await;

        peer.drain(|_| false).await.expect("the creation is sent");
        for _ in 0..3 {
            replicas.created((&peer, &shared));
        }
        let seals = replicas
            .asking()
            .into_iter()
            .filter(|asked| asked.contains("a seal"))
            .count();
        assert_eq!(seals, 1, "the replica is asked for its seal once");
    });
}
