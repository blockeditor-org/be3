use super::*;

use be_block_client::Event;

/// Another device renaming or sharing an open block is told on `changes` with the head the
/// replica already has, which must not reach it as a head that moved.
#[test]
fn news_of_a_block_whose_head_stayed_moves_no_replica() {
    with_test_types(|peer, shared, mut replicas| async move {
        let block = block(&peer).await;
        peer.save(block, &base(), None)
            .await
            .expect("the block is written");
        replicas
            .open((&peer, &shared), block, Sections::CONTENT_TYPE, true)
            .await;
        while replicas.joining() {
            replicas.pump((&peer, &shared)).await;
            tokio::task::yield_now().await;
        }
        let summary = peer.summary(block).await.expect("the block is read");
        assert!(summary.head.is_some());

        let renamed = Event::BlockChanged {
            block: summary.clone(),
            own: false,
        };
        replicas.heard((&peer, &shared), &renamed);
        replicas.heard((&peer, &shared), &renamed);
        assert_eq!(
            replicas.heads_told, 0,
            "a change that left the head as the replica has it moved it"
        );

        // A head this device never heard of, as another device's publish would name.
        let elsewhere = CommitId::from_hash(be_block_store::Hash::from_bytes([9; 32]));
        let moved = Event::BlockChanged {
            block: be_block_protocol::BlockSummary {
                head: Some(elsewhere),
                ..summary
            },
            own: false,
        };
        replicas.heard((&peer, &shared), &moved);
        assert_eq!(
            replicas.heads_told, 1,
            "a head another device published moved it"
        );
    });
}
