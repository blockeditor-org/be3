use be_type_text::TextBlock;

use super::*;

/// As `edits_typed_offline_while_a_reload_of_the_replicas_own_is_on_its_way_reach_the_server`,
/// with a fourth edit typed after the merge, while its reload is on its way and the join waits:
/// it goes on after the reload, and the seal the join waits for holds it with the three, so it
/// reaches the server once.
#[test]
fn an_edit_typed_while_a_reload_of_the_replicas_own_is_on_its_way_reaches_the_server_once() {
    with_test_types_on(
        be_block_client::Clock::manual(),
        |peer, shared, mut replicas| async move {
            let (block, base, connection, token, mut mirror) =
                text_typed_offline(&peer, &shared, &mut replicas, "base").await;
            let kept = match peer.save(block, &TextBlock::of("base!"), Some(base)).await {
                Ok(be_block_client::Saved::Kept(kept)) => kept,
                saved => panic!("the commit is kept offline: {saved:?}"),
            };
            replicas.moved_here(block, kept);

            peer.attach(connection, token)
                .await
                .expect("the device is connected again");
            replicas.reconnect((&peer, &shared));
            step_until_replicas(
                &peer,
                &shared,
                &mut replicas,
                "owed no seal with the edits unsealed while joining",
                |replicas, _| {
                    replicas.joining()
                        && replicas.status(block).is_some_and(|status| {
                            status.journaled_taken > status.sealed_taken && !status.owes_seal
                        })
                },
            )
            .await;
            receive_frames(&shared, &mut replicas, block, 7, &mut mirror);
            type_at_start(&mut replicas, block, 7, &mut mirror, "d");
            step_until_replicas(&peer, &shared, &mut replicas, "joined", |replicas, _| {
                !replicas.joining() && replicas.status(block).is_some()
            })
            .await;
            drain_all(&peer, &shared, &mut replicas, block).await;
            let text = text_on_the_server(&peer, block).await;
            assert_eq!(
                (text.matches('d').count(), text.matches("cbabase").count()),
                (1, 1),
                "the server's head holds the fourth edit and the three, each once: {text:?}"
            );
            assert!(text.contains("base!"), "and the kept commit: {text:?}");
            let errors = std::mem::take(&mut shared.lock().unwrap().errors);
            assert!(errors.is_empty(), "{errors:?}");
        },
    );
}
