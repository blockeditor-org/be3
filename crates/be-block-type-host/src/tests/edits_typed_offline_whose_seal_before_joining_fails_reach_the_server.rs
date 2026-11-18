use super::*;

/// Three edits typed offline, whose seal before joining fails (here as a store write refused for
/// a missing key would): the replica backs off and owes no seal, its edits journaled only in
/// this device's journal. The join's answer waits until a seal holds them, rather than opening
/// the replica on the server's journal, which would drop them; once the seal is tried again and
/// lands, they reach the server once.
#[test]
fn edits_typed_offline_whose_seal_before_joining_fails_reach_the_server() {
    with_test_types_on(
        be_block_client::Clock::manual(),
        |peer, shared, mut replicas| async move {
            let (block, _, connection, token, _) =
                text_typed_offline(&peer, &shared, &mut replicas, "base").await;
            replicas.failing_seals = 1;

            peer.attach(connection, token)
                .await
                .expect("the device is connected again");
            replicas.reconnect((&peer, &shared));
            step_until_replicas(
                &peer,
                &shared,
                &mut replicas,
                "held the join with the seal backing off",
                |replicas, _| {
                    replicas.joining()
                        && replicas.status(block).is_some_and(|status| {
                            !status.owes_seal && status.journaled_taken > status.sealed_taken
                        })
                },
            )
            .await;
            assert_eq!(replicas.failing_seals, 0, "the seal before joining failed");
            let joined = tokio::time::timeout(std::time::Duration::from_secs(30), async {
                loop {
                    let stepped = tokio::time::timeout(
                        std::time::Duration::from_millis(200),
                        step(&peer, &shared, &mut replicas),
                    )
                    .await;
                    if !replicas.joining() && replicas.status(block).is_some() {
                        break;
                    }
                    // Nothing more comes until the seal's back-off passes.
                    if stepped.is_err() {
                        replicas
                            .modules
                            .clock()
                            .advance(std::time::Duration::from_secs(10));
                    }
                }
            })
            .await;
            assert!(joined.is_ok(), "never joined");

            drain_all(&peer, &shared, &mut replicas, block).await;
            assert_eq!(
                text_on_the_server(&peer, block).await,
                "cbabase",
                "the server's head holds the three edits once"
            );
        },
    );
}
