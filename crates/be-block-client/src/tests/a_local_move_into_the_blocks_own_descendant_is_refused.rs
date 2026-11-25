use super::*;

use be_type_text::TextContent;

/// A local account's moves are never sent, so no server refuses one that would make a block its
/// own ancestor: the device refuses it as the server would, and the block stays where it was.
#[tokio::test]
async fn a_local_move_into_the_blocks_own_descendant_is_refused() {
    let peer = Peer::local(
        PeerConfig::new("", identity("local@example.com"), Credentials::Header)
            .chunker(ChunkerConfig::SMALL)
            .refs(refs::RefsDb::memory()),
        Uuid::now_v7(),
        Uuid::now_v7(),
        MemoryStore::new(),
    )
    .unwrap();
    let outer = peer
        .create::<TextContent>(BlockParent::Root)
        .await
        .unwrap();
    let inner = peer
        .create::<TextContent>(BlockParent::Block(outer))
        .await
        .unwrap();
    let below = peer
        .create::<TextContent>(BlockParent::Block(inner))
        .await
        .unwrap();

    for target in [outer, inner, below] {
        let moved = peer.set_parent(outer, BlockParent::Block(target)).await;
        assert!(
            matches!(moved, Err(ClientError::Refused(ErrorCode::ParentCycle, _))),
            "a move under itself is refused: {moved:?}"
        );
    }
    assert_eq!(
        peer.summary(outer).await.unwrap().parent,
        Some(BlockParent::Root),
        "the block stays where it was"
    );
    peer.set_parent(below, BlockParent::Block(outer))
        .await
        .expect("a move up is no cycle");
    assert_eq!(
        peer.summary(below).await.unwrap().parent,
        Some(BlockParent::Block(outer))
    );
}
