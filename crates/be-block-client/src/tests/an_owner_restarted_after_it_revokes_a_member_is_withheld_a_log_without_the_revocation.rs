use super::*;

/// An owner's device revokes a reader, and the server then withholds the revocation. The device
/// restarts on the same refs: the revocation it appended is kept with them, so the log served
/// without it is withheld still, the epoch the reader holds is outdated, and a save is refused
/// rather than sealed under it, until the server serves the revocation.
#[tokio::test]
async fn an_owner_restarted_after_it_revokes_a_member_is_withheld_a_log_without_the_revocation() {
    let harness = Harness::start().await;
    let admin = harness.owner("admin@example.com").await;
    let bob = harness.member(&admin, "bob@example.com").await;
    let team = workspace(&admin, "Team").await;
    let note = admin
        .create::<TextContent>(BlockParent::Block(team))
        .await
        .unwrap();
    let before = admin
        .save(note, &TextBlock::of("before"), None)
        .await
        .unwrap()
        .published();
    admin
        .grant(team, bob.account(), Access::View)
        .await
        .unwrap();
    admin.tend().await.unwrap();
    assert_eq!(text(&bob, note).await, "before");
    let old = admin.keys().scope_key(team).unwrap();

    let refs = refs::RefsDb::memory();
    let store = MemoryStore::new();
    let device = harness.device(&admin, &refs, &store).await;
    device
        .grant(team, bob.account(), Access::None)
        .await
        .unwrap();
    let withheld = withhold_newest(&harness, admin.organization(), Some(team));
    drop(device);

    let restarted = harness.device(&admin, &refs, &store).await;
    restarted.refresh_keys(None).await.unwrap();
    assert!(
        matches!(restarted.checked_access(team).await, Err(ClientError::Withheld(block)) if block == team),
        "a log served without the revocation this device appended is withheld"
    );
    assert!(
        restarted.keys().outdated(old, team),
        "the epoch the revoked reader holds is outdated"
    );
    let epochs = restarted.keys().keys_of(team, KeyKind::Scope).len();
    let data = restarted.keys().keys_of(note, KeyKind::Data).len();
    assert!(
        matches!(
            restarted.save(note, &TextBlock::of("after"), before).await,
            Err(ClientError::Locked(_))
        ),
        "a save while the revocation is withheld is refused"
    );
    assert_eq!(
        (
            restarted.keys().keys_of(team, KeyKind::Scope).len(),
            restarted.keys().keys_of(note, KeyKind::Data).len()
        ),
        (epochs, data),
        "a refused save mints no epoch"
    );
    assert_eq!(text(&harness.second(&admin).await, note).await, "before");

    withheld.serve(&harness);
    restarted
        .save(note, &TextBlock::of("after"), before)
        .await
        .unwrap();
    assert_eq!(
        restarted.keys().keys_of(team, KeyKind::Scope).len(),
        epochs + 1,
        "the save once the revocation is served gives the scope a new epoch"
    );
    let written = restarted
        .keys()
        .keys_of(note, KeyKind::Data)
        .last()
        .unwrap()
        .id;
    bob.refresh_keys(None).await.unwrap();
    assert!(!bob.keys().has(written));
    assert_eq!(text(&harness.second(&admin).await, note).await, "after");

    harness.stop().await;
}
