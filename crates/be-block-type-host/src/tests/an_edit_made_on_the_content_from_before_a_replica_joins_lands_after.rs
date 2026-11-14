use be_block_content::BlockContent;
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

/// A replica that joins its session reopens on its head, and an editor's next edit is numbered
/// by the content it was shown before: the session's state as of that head (where each writer's
/// next edit starts) comes from the journal on it, this device's when the server's is on
/// another commit, so the edit lands rather than being taken for one out of order.
#[test]
fn an_edit_made_on_the_content_from_before_a_replica_joins_lands_after() {
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
        let mut mirror =
            attach_mirror::<TextContent>(&peer, &shared, &mut replicas, block, editor).await;
        type_at_start(&mut replicas, block, editor, &mut mirror, "socks");
        replicas.pump((&peer, &shared)).await;
        replicas.keep_all((&peer, &shared)).await;
        replicas.pump((&peer, &shared)).await;
        // What the editor is shown before the replica joins.
        receive_frames(&shared, editor, &mut mirror);
        assert_eq!(TextBlock::text(mirror.content().expect("shown")), "socks");

        peer.drain(|_| false).await.expect("the creation is sent");
        replicas.created((&peer, &shared));
        assert!(replicas.joining(), "once sent, the block joins its session");
        while replicas.joining() {
            replicas.woken().await;
            replicas.pump((&peer, &shared)).await;
        }
        replicas.pump((&peer, &shared)).await;

        type_at_start(&mut replicas, block, editor, &mut mirror, "boots ");
        replicas.pump((&peer, &shared)).await;
        assert_eq!(
            text(&peer, &shared, &mut replicas, block).await,
            "boots socks",
            "the edit made on the content shown before the replica joined lands"
        );
    });
}
