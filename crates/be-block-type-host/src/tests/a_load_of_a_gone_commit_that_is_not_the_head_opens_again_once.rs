use super::*;

/// A replica whose load found the objects of a commit that is not the head gone (a merge's
/// side, a restart's commit, a head the server dropped as another replaced it) opens again at
/// once, which loads afresh from the head; the same commit missing again then is unreadable, as
/// opening again would only load it again, round and round.
#[test]
fn a_load_of_a_gone_commit_that_is_not_the_head_opens_again_once() {
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
        let gone = CommitId::from_hash(be_block_store::Hash::from_bytes([7; 32]));

        replicas.load_found_missing(&peer, block, gone);
        while openings(&replicas) == opened {
            replicas.woken().await;
            replicas.pump((&peer, &shared)).await;
        }
        while replicas.joining() {
            replicas.pump((&peer, &shared)).await;
            tokio::task::yield_now().await;
        }

        replicas.load_found_missing(&peer, block, gone);
        for _ in 0..4 {
            replicas.pump((&peer, &shared)).await;
            tokio::task::yield_now().await;
        }
        assert_eq!(
            openings(&replicas),
            opened + 1,
            "the same commit missing again opened the replica again"
        );
    });
}
