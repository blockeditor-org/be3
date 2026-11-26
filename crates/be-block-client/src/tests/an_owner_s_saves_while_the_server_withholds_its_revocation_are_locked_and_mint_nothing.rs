use super::*;

/// An owner revokes a reader, and the server then withholds the revocation from the team's log.
/// The owner's saves are refused as locked: none is written under the epoch the reader holds,
/// and none rotates, since an epoch minted on a log short of the revocation would be outdated at
/// once, so repeated saves leave no epochs behind. Once the server serves the revocation, the
/// next save gives the scope a new epoch the reader never gets.
#[tokio::test]
async fn an_owner_s_saves_while_the_server_withholds_its_revocation_are_locked_and_mint_nothing() {
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
    bob.refresh_keys(None).await.unwrap();
    let held = bob.keys().keys_of(note, KeyKind::Data).len();

    admin
        .grant(team, bob.account(), Access::None)
        .await
        .unwrap();
    let withheld = withhold_newest(&harness, admin.organization(), Some(team));
    let epochs = admin.keys().keys_of(team, KeyKind::Scope).len();
    let data = admin.keys().keys_of(note, KeyKind::Data).len();
    for _ in 0..3 {
        assert!(
            matches!(
                admin.save(note, &TextBlock::of("after"), before).await,
                Err(ClientError::Locked(_))
            ),
            "a save while the revocation is withheld is refused"
        );
    }
    assert_eq!(
        (
            admin.keys().keys_of(team, KeyKind::Scope).len(),
            admin.keys().keys_of(note, KeyKind::Data).len()
        ),
        (epochs, data),
        "refused saves mint no epoch"
    );
    bob.refresh_keys(None).await.unwrap();
    assert_eq!(
        bob.keys().keys_of(note, KeyKind::Data).len(),
        held,
        "nothing new is sealed to the revoked reader"
    );
    assert_eq!(text(&harness.second(&admin).await, note).await, "before");

    withheld.serve(&harness);
    admin
        .save(note, &TextBlock::of("after"), before)
        .await
        .unwrap();
    assert_eq!(
        admin.keys().keys_of(team, KeyKind::Scope).len(),
        epochs + 1,
        "the save once the revocation is served gives the scope a new epoch"
    );
    let written = admin.keys().keys_of(note, KeyKind::Data).last().unwrap().id;
    bob.refresh_keys(None).await.unwrap();
    assert!(!bob.keys().has(written));
    assert_eq!(text(&harness.second(&admin).await, note).await, "after");

    harness.stop().await;
}
