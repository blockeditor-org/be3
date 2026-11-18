use super::*;

/// A block closed while its replica's rejoin is held, for journaled edits no seal would take now,
/// lets go of the join rather than open again on the server's journal, which would lose those
/// edits, and closes, rather than wait for a status that may never come.
#[test]
fn a_block_closed_while_its_rejoin_is_held_lets_go_of_the_join_and_closes() {
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

        replicas.rejoined_after_a_seal(block, 3, 1, false);
        replicas.pump((&peer, &shared)).await;
        assert!(replicas.joining(), "the join's answer is held");

        replicas.close(block);
        replicas.pump((&peer, &shared)).await;
        assert!(!replicas.joining(), "closing lets go of the join");
        assert!(
            !replicas.seal_asked(block),
            "and asks no seal that could not take the edits"
        );
        while replicas.is_open(block) {
            replicas.woken().await;
            replicas.pump((&peer, &shared)).await;
        }
        assert_eq!(
            shared.lock().unwrap().released,
            vec![block],
            "the block closes, and is not opened again on the server's journal"
        );
    });
}
