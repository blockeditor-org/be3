use super::*;

/// Another device renaming or sharing an open block changes its tree with the head the replica
/// already has, which must not reach it as a head that moved.
#[test]
fn news_of_a_block_whose_head_stayed_moves_no_replica() {
    with_test_types(|peer, shared, mut replicas| async move {
        let block = block(&peer).await;
        peer.save(block, &base(), None)
            .await
            .expect("the block is written");
        replicas
            .open((&peer, &shared), block, Sections::CONTENT_TYPE, true)
            .await
            .expect("the block opens");
        while replicas.joining() {
            replicas.pump((&peer, &shared)).await;
            tokio::task::yield_now().await;
        }
        let summary = peer.summary(block).await.expect("the block is read");
        assert!(summary.head.is_some());

        let head = summary.head.unwrap();
        replicas.heard_head((&peer, &shared), block, head);
        replicas.heard_head((&peer, &shared), block, head);
        assert_eq!(
            replicas.heads_told, 0,
            "a change that left the head as the replica has it moved it"
        );

        // A head this device never heard of, as another device's publish would name.
        let elsewhere = CommitId::from_hash(be_block_store::Hash::from_bytes([9; 32]));
        replicas.heard_head((&peer, &shared), block, elsewhere);
        assert_eq!(
            replicas.heads_told, 1,
            "a head another device published moved it"
        );
    });
}
