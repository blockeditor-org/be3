use super::*;

/// A block asked to join its session again as it closes (a host component's release, after its
/// editor closed, plans/hosted_blocks.md, step 4d2) starts no join, which would hold up the
/// close and so the block opening again: it closes, and leaving drops any claim it made.
#[test]
fn a_closing_block_is_not_joined_again_and_closes() {
    with_test_types(async |peer, shared, mut replicas| {
        let block = block(&peer).await;
        replicas
            .open((&peer, &shared), block, Sections::CONTENT_TYPE, false)
            .await
            .expect("the block opens");
        while replicas.status(block).is_none() {
            replicas.woken().await;
            replicas.pump((&peer, &shared)).await;
        }
        replicas.pump((&peer, &shared)).await;

        replicas.close(block);
        replicas.rejoin_block((&peer, &shared), block);
        assert!(!replicas.joining(), "a closing block starts no join");
        while replicas.is_open(block) {
            replicas.woken().await;
            replicas.pump((&peer, &shared)).await;
        }
        assert_eq!(
            shared.lock().unwrap().released,
            vec![block],
            "the block closes"
        );
    });
}
