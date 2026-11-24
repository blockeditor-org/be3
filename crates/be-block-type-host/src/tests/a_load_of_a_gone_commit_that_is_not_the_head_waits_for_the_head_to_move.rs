use super::*;

/// A replica whose load found a commit's objects gone waits for the block's head to move on from
/// the one this device knew as it loaded, not from the commit it loaded: one that is not the head
/// (a merge's side, a restart's commit) differs from the head at once, and opening again on it
/// would only fail again, round and round. Once another head is told, it opens again.
#[test]
fn a_load_of_a_gone_commit_that_is_not_the_head_waits_for_the_head_to_move() {
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
        let openings = |replicas: &Replicas<Quiet>| {
            replicas
                .openings
                .iter()
                .filter(|(opened, ..)| *opened == block)
                .count()
        };
        let opened = openings(&replicas);

        replicas.load_found_missing(&peer, block);
        for _ in 0..4 {
            replicas.pump((&peer, &shared)).await;
            tokio::task::yield_now().await;
        }
        assert_eq!(
            openings(&replicas),
            opened,
            "a missing commit opened the replica again with the head where it was"
        );

        // A head this device never heard of, as another device's publish would name.
        let elsewhere = CommitId::from_hash(be_block_store::Hash::from_bytes([9; 32]));
        replicas.heard_head((&peer, &shared), block, elsewhere);
        while openings(&replicas) == opened {
            replicas.woken().await;
            replicas.pump((&peer, &shared)).await;
        }
    });
}
