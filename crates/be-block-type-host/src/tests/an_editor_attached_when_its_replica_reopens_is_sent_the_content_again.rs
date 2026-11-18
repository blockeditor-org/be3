use be_block_content::BlockContent;
use be_type_text::TextContent;

use super::*;

/// A replica opened again in place of one that ended (as keys arrive, or as an unreadable
/// block is reset) takes the editors the first had: they are sent its content, not left on
/// what the ended one last told them.
#[test]
fn an_editor_attached_when_its_replica_reopens_is_sent_the_content_again() {
    with_test_types(|peer, shared, mut replicas| async move {
        let block = peer
            .create::<TextContent>(BlockParent::Root)
            .await
            .expect("the text is made");
        replicas
            .open((&peer, &shared), block, TextContent::CONTENT_TYPE, true)
            .await
            .expect("the block opens");
        replicas.pump((&peer, &shared)).await;
        let editor = 7;
        let mut mirror =
            attach_mirror::<TextContent>(&peer, &shared, &mut replicas, block, editor).await;
        assert!(mirror.is_loaded());
        let before = mirror.revision();

        replicas.reopen(block);
        while mirror.revision() == before {
            step(&peer, &shared, &mut replicas).await;
            receive_frames(&shared, &mut replicas, block, editor, &mut mirror);
        }
        assert!(
            mirror.is_loaded(),
            "the editor is sent the reopened replica's content"
        );
        assert!(replicas.is_open(block));
    });
}
