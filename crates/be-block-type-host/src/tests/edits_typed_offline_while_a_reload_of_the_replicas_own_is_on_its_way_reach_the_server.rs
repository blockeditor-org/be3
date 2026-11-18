use be_type_text::TextBlock;

use super::*;

/// Three edits typed offline, and a commit this device kept on the block's head meanwhile, which
/// its replica reconciles with as it reconnects: the reload of that merge is on its way when the
/// seal before joining is asked, so the seal seals nothing (it would publish on the head the
/// reload replaces) and the replica owes none until the reload lands. The edits are not lost:
/// the merge it reloads holds them, beside the kept commit, and both reach the server.
#[test]
fn edits_typed_offline_while_a_reload_of_the_replicas_own_is_on_its_way_reach_the_server() {
    with_test_types_on(
        be_block_client::Clock::manual(),
        |peer, shared, mut replicas| async move {
            let (block, base, connection, token) =
                text_typed_offline(&peer, &shared, &mut replicas).await;
            let kept = match peer.save(block, &TextBlock::of("base!"), Some(base)).await {
                Ok(be_block_client::Saved::Kept(kept)) => kept,
                saved => panic!("the commit is kept offline: {saved:?}"),
            };
            replicas.moved_here(block, kept);

            peer.attach(connection, token)
                .await
                .expect("the device is connected again");
            replicas.reconnect((&peer, &shared));
            let owed_none = std::cell::Cell::new(false);
            step_until_replicas(&peer, &shared, &mut replicas, "joined", |replicas, _| {
                let status = replicas.status(block);
                if replicas.joining()
                    && let Some(status) = &status
                    && status.journaled_taken > status.sealed_taken
                    && !status.owes_seal
                {
                    owed_none.set(true);
                }
                !replicas.joining() && status.is_some()
            })
            .await;
            assert!(
                owed_none.get(),
                "while joining, the replica owed no seal with the edits unsealed"
            );

            drain_all(&peer, &shared, &mut replicas, block).await;
            let text = text_on_the_server(&peer, block).await;
            assert_eq!(
                text.matches("cbabase").count(),
                1,
                "the server's head holds the three edits once: {text:?}"
            );
            assert!(text.contains("base!"), "and the kept commit: {text:?}");
            let errors = std::mem::take(&mut shared.lock().unwrap().errors);
            assert!(errors.is_empty(), "{errors:?}");
        },
    );
}
