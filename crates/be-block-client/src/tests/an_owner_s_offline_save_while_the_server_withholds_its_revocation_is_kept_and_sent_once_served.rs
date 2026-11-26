use super::*;

/// An owner revokes a reader, the server withholds the revocation from the team's log, and the
/// owner's device restarts offline. Its save is kept for the outbox, not refused, though no
/// epoch may be minted while the log is short of the revocation. Connected again with the log
/// still withheld, the drain tries the edit again later rather than send it under the epoch the
/// reader holds or set it aside; once the server serves the revocation, the edit is sealed
/// again under a new epoch the reader never gets.
#[tokio::test]
async fn an_owner_s_offline_save_while_the_server_withholds_its_revocation_is_kept_and_sent_once_served()
 {
    let harness = Harness::start().await;
    let admin = harness.owner("admin@example.com").await;
    let bob = harness.member(&admin, "bob@example.com").await;
    let team = workspace(&admin, "Team").await;
    let note = admin
        .create::<TextContent>(BlockParent::Block(team))
        .await
        .unwrap();
    admin
        .grant(team, bob.account(), Access::View)
        .await
        .unwrap();
    admin.tend().await.unwrap();
    let refs = refs::RefsDb::memory();
    let store = MemoryStore::new();
    let laptop = harness.device(&admin, &refs, &store).await;
    let before = laptop
        .save(note, &TextBlock::of("before"), None)
        .await
        .unwrap()
        .published();
    assert_eq!(text(&bob, note).await, "before");
    laptop
        .grant(team, bob.account(), Access::None)
        .await
        .unwrap();
    let withheld = withhold_newest(&harness, admin.organization(), Some(team));
    drop(laptop);

    let offline = harness.offline(&admin, &refs, &store);
    let epochs = offline.keys().keys_of(team, KeyKind::Scope).len();
    assert!(
        matches!(
            offline
                .save(note, &TextBlock::of("after"), before)
                .await
                .unwrap(),
            Saved::Kept(_)
        ),
        "an offline save while the revocation is awaited is kept"
    );
    harness.reconnect(&offline, admin.token()).await;
    assert_eq!(
        offline.drain(|_| false).await.unwrap(),
        [Drained::Failed { block: note }],
        "the edit was not tried again later"
    );
    assert!(offline.unsent().is_empty(), "the edit was set aside");
    assert_eq!(
        offline.keys().keys_of(team, KeyKind::Scope).len(),
        epochs,
        "an epoch was minted while the revocation is withheld"
    );
    assert_eq!(text(&harness.second(&admin).await, note).await, "before");

    withheld.serve(&harness);
    let drained = offline.drain(|_| false).await.unwrap();
    assert!(
        drained.contains(&Drained::Resealed { block: note }),
        "the edit was not sealed again: {drained:?}"
    );
    assert_eq!(
        offline.keys().keys_of(team, KeyKind::Scope).len(),
        epochs + 1
    );
    let head = offline.local_head(note).unwrap();
    let key = offline
        .commits()
        .vault()
        .sealed_under(head.hash())
        .unwrap();
    bob.refresh_keys(None).await.unwrap();
    assert!(!bob.keys().has(key));
    assert_eq!(text(&harness.second(&admin).await, note).await, "after");

    harness.stop().await;
}
