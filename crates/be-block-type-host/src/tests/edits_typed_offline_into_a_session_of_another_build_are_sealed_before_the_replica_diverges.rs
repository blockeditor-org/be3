use super::*;

/// A replica diverges when its session speaks another build of the block's type: here another
/// device joined the block's session first with another build's tag while this one typed three
/// edits offline. Reconnecting seals them in the made-up session before the replica joins (the
/// rejoin waits for that seal), so the replica that diverges holds no journaled edits that no
/// seal holds, and the three reach the server in the commit that seal left in the outbox. A
/// diverged replica refuses edits, so it cannot journal any after.
#[test]
fn edits_typed_offline_into_a_session_of_another_build_are_sealed_before_the_replica_diverges() {
    with_devices(
        true,
        be_block_client::Clock::manual(),
        |peer, another, shared, mut replicas| async move {
            let another = another.expect("another device signed in");
            let (block, _, connection, token, _) =
                text_typed_offline(&peer, &shared, &mut replicas, "base").await;
            another
                .join_session(block, be_block_store::Hash::of(b"another build"))
                .await
                .expect("the other device joins first");

            peer.attach(connection, token)
                .await
                .expect("the device is connected again");
            replicas.reconnect((&peer, &shared));
            step_until_replicas(&peer, &shared, &mut replicas, "diverged", |replicas, _| {
                !replicas.joining() && replicas.status(block).is_some_and(|s| s.diverged)
            })
            .await;
            let status = replicas.status(block).expect("the replica reported");
            assert_eq!(
                (status.sealed_taken, status.journaled_taken),
                (0, 0),
                "the diverged replica holds no journaled edits"
            );
            assert_eq!(
                peer.pending_commits(block).len(),
                1,
                "the seal before joining holds the three edits"
            );

            drain_all(&peer, &shared, &mut replicas, block).await;
            assert_eq!(
                text_on_the_server(&peer, block).await,
                "cbabase",
                "the server's head holds the three edits once"
            );
            let errors = std::mem::take(&mut shared.lock().unwrap().errors);
            assert!(errors.is_empty(), "{errors:?}");
        },
    );
}
