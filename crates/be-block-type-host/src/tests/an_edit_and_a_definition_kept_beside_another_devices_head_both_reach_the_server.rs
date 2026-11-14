use be_block_client::{Drained, Saved};

use super::*;

/// An edit sealed beside a definition still in the outbox, both on a head another device has
/// moved on from since: the drain finds the block diverged, and the merges that follow put both
/// on the server with the other device's change. The merge of the edit waits in the outbox behind
/// the definition, and is not taken as on the server meanwhile.
#[test]
fn an_edit_and_a_definition_kept_beside_another_devices_head_both_reach_the_server() {
    with_another_device(|peer, phone, shared, mut replicas| async move {
        let (block, head, _, _) = sealed_beside_a_definition(&peer).await;
        let mut theirs = base();
        theirs.set(5, "FIVE");
        assert!(
            matches!(
                phone.save(block, &theirs, Some(head)).await,
                Ok(Saved::Published(_))
            ),
            "the other device moves the head on"
        );

        for _ in 0..4 {
            if peer.pending_commits(block).is_empty() {
                break;
            }
            let drained = peer.drain(|_| false).await.expect("the outbox drains");
            for outcome in drained {
                if let Drained::Diverged { remote, .. } = outcome {
                    replicas
                        .merge_diverged((&peer, &shared), block, remote)
                        .await
                        .expect("the block merges");
                }
            }
        }
        assert!(
            peer.pending_commits(block).is_empty(),
            "everything this device kept is sent"
        );
        assert_eq!(
            on_the_server(&peer, block, &[2, 5]).await,
            (
                Some("Renamed".to_owned()),
                vec![Some(b"TWO".to_vec()), Some(b"FIVE".to_vec())]
            ),
            "the server's head holds the definition, the edit and the other device's change"
        );
    });
}
