use be_block_content::BlockContent;
use be_type_text::TextContent;

use super::*;

/// Kept edits that stop opening between `open` checking them and the replica starting send the
/// block through `open` again, which sorts them out as it does any: here they open, so the
/// block opens rather than waiting, locked, for keys nothing will send.
#[test]
fn a_block_whose_kept_edits_stop_opening_as_its_replica_starts_opens_again_unlocked() {
    with_test_types(|peer, shared, mut replicas| async move {
        let block = peer
            .create::<TextContent>(BlockParent::Root)
            .await
            .expect("the text is made");
        replicas.kept_stop_opening.insert(block);
        replicas
            .open((&peer, &shared), block, TextContent::CONTENT_TYPE, true)
            .await
            .expect("the block opens");
        assert!(replicas.is_open(block), "the block opened on the second pass");
        assert!(
            !shared.lock().unwrap().locked.contains(&block),
            "the block was not left locked"
        );
    });
}
