use super::*;

/// A replica whose join's answer came back after its first seal, still holding journaled edits
/// no seal holds while no seal would take them now (it diverged, or its seal backs off or cannot
/// merge), keeps the answer held and is not asked for a seal again on every pass; once its
/// status says a seal would take them, it is asked once more.
#[test]
fn a_rejoin_whose_unsealed_edits_no_seal_can_take_waits_without_asking_again() {
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
        for _ in 0..5 {
            replicas.pump((&peer, &shared)).await;
            assert!(
                !replicas.seal_asked(block),
                "no seal is asked while none would take the edits"
            );
            assert!(replicas.joining(), "the join's answer stays held");
        }

        replicas.set_owes_seal(block, true);
        replicas.pump((&peer, &shared)).await;
        assert!(
            replicas.seal_asked(block),
            "once a seal would take them, one is asked"
        );
        assert!(replicas.joining(), "and the answer waits for it");
    });
}
