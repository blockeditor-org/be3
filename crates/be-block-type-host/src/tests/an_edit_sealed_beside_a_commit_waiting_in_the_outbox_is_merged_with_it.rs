use be_block_client::Drained;

use super::*;

/// An edit sealed on the block's server head while a definition this device kept on that same
/// head still waits in the outbox goes after it, not ahead: the server refuses it as stale once
/// the definition is in, and the merge takes in both, rather than the edit replacing the
/// definition it never saw.
#[test]
fn an_edit_sealed_beside_a_commit_waiting_in_the_outbox_is_merged_with_it() {
    with_test_types(async |peer, shared, mut replicas| {
        let (block, _, definition, edit) = sealed_beside_a_definition(&peer).await;

        let drained = peer.drain(|_| false).await.expect("the outbox drains");
        assert!(
            drained.contains(&Drained::Published {
                block,
                head: definition,
            }),
            "the definition goes first: {drained:?}"
        );
        assert!(
            drained.contains(&Drained::Diverged {
                block,
                local: edit,
                remote: Some(definition),
            }),
            "the edit is refused as stale: {drained:?}"
        );

        replicas
            .merge_diverged((&peer, &shared), block, Some(definition))
            .await
            .expect("the edit merges with the definition")
            .expect("the merge reaches the server");
        assert!(
            peer.pending_commits(block).is_empty(),
            "the merge settles the edit"
        );
        assert_eq!(
            on_the_server(&peer, block, &[2]).await,
            (Some("Renamed".to_owned()), vec![Some(b"TWO".to_vec())]),
            "the server's head holds both the definition and the edit"
        );
    });
}
