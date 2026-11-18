use super::*;

/// A device kept every entry of the team's log from its last lowering through its head:
/// offline, a key naming one between them, made after the revocation and before a later grant,
/// is current, and the save goes through.
#[tokio::test]
async fn an_offline_save_under_a_key_named_between_the_last_lowering_and_the_head_goes_through() {
    let harness = Harness::start().await;
    let admin = harness.owner("admin@example.com").await;
    let carol = harness.member(&admin, "carol@example.com").await;
    let bob = harness.member(&admin, "bob@example.com").await;
    let dave = harness.member(&admin, "dave@example.com").await;
    let erin = harness.member(&admin, "erin@example.com").await;
    let team = workspace(&admin, "Team").await;
    let note = admin
        .create::<TextContent>(BlockParent::Block(team))
        .await
        .unwrap();
    let head = admin
        .save(note, &TextBlock::of("before"), None)
        .await
        .unwrap()
        .published();
    for (member, access) in [(&carol, Access::Edit), (&bob, Access::View)] {
        admin.grant(team, member.account(), access).await.unwrap();
    }
    admin.tend().await.unwrap();
    // The revocation, a grant, the rotation (naming the grant's head), then another grant.
    admin
        .grant(team, bob.account(), Access::None)
        .await
        .unwrap();
    admin
        .grant(team, dave.account(), Access::View)
        .await
        .unwrap();
    admin.tend().await.unwrap();
    let (entries, _) = admin.checked_access(team).await.unwrap();
    let between = entries.last().unwrap().hash();
    admin
        .grant(team, erin.account(), Access::View)
        .await
        .unwrap();

    let refs = refs::RefsDb::memory();
    let store = MemoryStore::new();
    let online = harness.device(&carol, &refs, &store).await;
    assert_eq!(text(&online, note).await, "before");
    let current = online
        .keys()
        .keys_of(note, KeyKind::Data)
        .last()
        .unwrap()
        .id;
    assert_eq!(online.keys().heads(current).block, Some(between));
    drop(online);

    let offline = harness.offline(&carol, &refs, &store);
    let saved = offline.save(note, &TextBlock::of("offline"), head).await;
    assert!(
        saved.is_ok(),
        "an offline save under a current key failed: {saved:?}"
    );

    harness.stop().await;
}
