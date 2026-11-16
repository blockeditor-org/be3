use be_block_content::BlockContent;
use be_type_text::{TextBlock, TextContent};

use super::*;

/// A block made on this device takes an edit that is sealed, and then one this device only
/// numbered, before the server has the block. Once it is sent, the replica joins its session
/// and opens again on what this device numbered: the second edit is kept, and sealed.
#[test]
fn an_edit_numbered_offline_after_a_seal_survives_joining() {
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
            .await
            .expect("the block opens");
        replicas.pump((&peer, &shared)).await;
        let editor = 7;
        let mut mirror =
            attach_mirror::<TextContent>(&peer, &shared, &mut replicas, block, editor).await;
        type_at_start(&mut replicas, block, editor, &mut mirror, "socks");
        replicas.pump((&peer, &shared)).await;
        replicas.keep_all((&peer, &shared)).await;
        replicas.pump((&peer, &shared)).await;
        receive_frames(&shared, editor, &mut mirror);
        type_at_start(&mut replicas, block, editor, &mut mirror, "boots ");
        replicas.pump((&peer, &shared)).await;

        peer.drain(|_| false).await.expect("the creation is sent");
        replicas.created((&peer, &shared));
        while replicas.joining() {
            step(&peer, &shared, &mut replicas).await;
        }
        replicas.pump((&peer, &shared)).await;
        let bytes = replicas
            .content((&peer, &shared), block)
            .await
            .expect("the text is open");
        let text = TextBlock::text(&TextContent::decode(&bytes).expect("the text decodes"));
        assert_eq!(
            text, "boots socks",
            "the edit only this device numbered is kept"
        );
        while replicas.status(block).is_none_or(|status| !status.clean) {
            step(&peer, &shared, &mut replicas).await;
        }
    });
}
