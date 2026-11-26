use super::*;

/// An owner's device checked the team's log in an earlier run; meanwhile another device of the
/// owner's revokes a reader. Restarted offline, the device's save cannot read the log, so it is
/// sealed under the epoch at hand by what was kept and kept for the outbox; the failed read
/// does not count as this run's read, so the drain once connected reads the log, sees the
/// revocation, and seals the edit again under a new epoch the revoked reader never gets.
#[tokio::test]
async fn an_offline_save_whose_log_read_failed_reads_it_again_before_it_is_sent() {
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
    bob.refresh_keys(None).await.unwrap();
    drop(laptop);
    admin
        .grant(team, bob.account(), Access::None)
        .await
        .unwrap();

    let offline = harness.offline(&admin, &refs, &store);
    let epochs = offline.keys().keys_of(team, KeyKind::Scope).len();
    assert!(matches!(
        offline
            .save(note, &TextBlock::of("after"), before)
            .await
            .unwrap(),
        Saved::Kept(_)
    ));
    harness.reconnect(&offline, admin.token()).await;
    let drained = offline.drain(|_| false).await.unwrap();
    assert!(
        drained.contains(&Drained::Resealed { block: note }),
        "the edit was sent judged by the log kept from before: {drained:?}"
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
    assert!(!bob.keys().has(key), "the edit was sent under a key the revoked reader holds");
    harness.stop().await;
}
