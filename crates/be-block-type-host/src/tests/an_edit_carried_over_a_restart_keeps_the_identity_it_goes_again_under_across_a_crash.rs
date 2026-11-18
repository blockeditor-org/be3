use super::*;

/// An edit is on its way when the order restarts (the holder sealed a session state past
/// `RESTART_STATE_BYTES`): the server numbers it after the restart, every replica passes it over,
/// and its replica submits it again carried over. The host keeps that submission's identity,
/// though the replica's status naming the restart comes in the same pass, so a replica opened
/// after a crash would give the edit again under it; and the edit lands once.
#[test]
fn an_edit_carried_over_a_restart_keeps_the_identity_it_goes_again_under_across_a_crash() {
    with_test_types(|peer, shared, mut replicas| async move {
        let (block, added) = carry_an_edit_over_a_restart(&peer, &shared, &mut replicas).await;
        drop(replicas);
        let mut replicas = Replicas::new(test_modules(be_block_client::Clock::real()).await);
        replicas
            .open((&peer, &shared), block, Tally::CONTENT_TYPE, true)
            .await
            .expect("the block opens");
        // A replica reports no restarts until it loads: that drops none of what it was given,
        // before it submits them again.
        pump_until(
            &peer,
            &shared,
            &mut replicas,
            "heard from the reopened replica",
            |replicas| {
                replicas
                    .submissions_of(block)
                    .is_some_and(|(_, restarts, _)| restarts == 0)
            },
        )
        .await;
        let (_, _, kept) = replicas
            .submissions_of(block)
            .expect("the block is open again");
        assert_eq!(
            kept,
            [added],
            "the reopened replica's first status kept them"
        );
        assert_eq!(
            replicas.kept_restarts_of(block),
            Some(1),
            "what is kept before the replica loads is kept under the restart it was made under"
        );
        while replicas.joining() {
            replicas.woken().await;
            replicas.pump((&peer, &shared)).await;
        }
        step_until_replicas(
            &peer,
            &shared,
            &mut replicas,
            "settled the edit",
            |replicas, _| {
                replicas
                    .submissions_of(block)
                    .is_some_and(|(submitting, ..)| submitting == 0)
            },
        )
        .await;
        let bytes = replicas
            .content((&peer, &shared), block)
            .await
            .expect("the tally is open");
        let tally = Tally::decode(&bytes).expect("the tally decodes");
        assert_eq!(tally.count, 10, "the edit landed once");
        let errors = std::mem::take(&mut shared.lock().unwrap().errors);
        assert!(errors.is_empty(), "{errors:?}");
    });
}
