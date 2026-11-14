use be_block_client::{Drained, Saved, Sealed, Write};

use super::*;

/// An edit sealed on the block's server head while a definition this device kept on that same
/// head still waits in the outbox goes after it, not ahead: the server refuses it as stale once
/// the definition is in, and the merge takes in both, rather than the edit replacing the
/// definition it never saw.
#[test]
fn an_edit_sealed_beside_a_commit_waiting_in_the_outbox_is_merged_with_it() {
    with_test_types(async |peer, shared, mut replicas| {
        let block = block(&peer).await;
        let head = match peer.save(block, &base(), None).await {
            Ok(Saved::Published(head)) => head,
            saved => panic!("the base publishes: {saved:?}"),
        };
        peer.keep_definition(block, |metadata| {
            metadata.insert(&be_block_content::NAME, &"Renamed".to_owned());
        })
        .await
        .expect("the definition is kept here");
        let definition = peer.local_head(block).expect("the definition is sealed");
        assert_ne!(definition, head);

        let edited = written(&peer, block, &[(2, "TWO")]).await;
        let edit = match peer
            .seal(block, Write::written(edited, Some(head)))
            .await
            .expect("the edit is sealed")
        {
            Sealed::Sealed(edit) => edit,
            sealed => panic!("the edit changes the block: {sealed:?}"),
        };
        let published = peer
            .publish_sealed(block, edit)
            .await
            .expect("the edit is kept");
        assert_eq!(
            published,
            Saved::Unchanged(edit),
            "the edit waits behind the definition sealed before it"
        );

        let drained = peer.drain(|_| false).await.expect("the outbox drains");
        assert!(
            drained.contains(&Drained::Published {
                block,
                head: definition,
            }),
            "the definition goes first: {drained:?}"
        );
        let remote = drained
            .iter()
            .find_map(|outcome| match *outcome {
                Drained::Diverged {
                    block: diverged,
                    local,
                    remote,
                } if diverged == block => Some((local, remote)),
                _ => None,
            })
            .unwrap_or_else(|| panic!("the edit is refused as stale: {drained:?}"));
        assert_eq!(remote, (edit, Some(definition)));

        replicas
            .merge_diverged((&peer, &shared), block, Some(definition))
            .await
            .expect("the edit merges with the definition");
        assert!(
            peer.pending_commits(block).is_empty(),
            "the merge settles the edit"
        );
        let head = peer
            .remote_head(block)
            .await
            .expect("the head is read")
            .expect("the block has a head");
        let name = peer
            .metadata_of(head)
            .await
            .expect("the metadata is read")
            .get(&be_block_content::NAME);
        let manifest = peer
            .load_commit(head)
            .await
            .expect("the head is read")
            .manifest;
        let stored = peer
            .load_manifest(&manifest)
            .await
            .expect("the head's content is read");
        let content = Sections::from_stored(&stored).expect("the sections decode");
        assert_eq!(
            (name.as_deref(), content.section(2)),
            (Some("Renamed"), Some(&b"TWO"[..])),
            "the server's head holds both the definition and the edit"
        );
    });
}
