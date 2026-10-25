use super::*;

use be_graph::Access;
use text_block_types::{TextBlock, TextContent};

#[tokio::test]
async fn a_reader_taken_off_a_workspace_cannot_read_what_is_written_after() {
    let harness = Harness::start().await;
    let owner = harness.owner("owner@example.com").await;
    let reader = harness.member(&owner, "reader@example.com").await;
    let leaver = harness.member(&owner, "leaver@example.com").await;
    let team = workspace(&owner, "Team").await;
    let note = owner
        .create::<TextContent>(BlockParent::Block(team))
        .await
        .unwrap();
    let before = owner
        .save(note, &TextBlock::of("before"), None)
        .await
        .unwrap()
        .published()
        .unwrap();
    for member in [&reader, &leaver] {
        let public = owner
            .keys()
            .contact(member.account())
            .expect("the invitation made them contacts");
        owner
            .share(team, member.account(), Access::Edit, &public)
            .await
            .unwrap();
        assert_eq!(text(member, note).await, "before");
    }
    let old_key = owner.summary(note).await.unwrap().key.unwrap();

    owner
        .grant(team, leaver.account(), Access::None)
        .await
        .unwrap();
    assert_eq!(owner.key_work().await.unwrap().rotations, [team]);
    owner.tend().await.unwrap();
    assert!(owner.key_work().await.unwrap().rotations.is_empty());
    let new_key = owner.summary(note).await.unwrap().key.unwrap();
    assert_ne!(new_key, old_key, "the block kept writing with the old key");
    let after = owner
        .save(note, &TextBlock::of("after"), Some(before))
        .await
        .unwrap()
        .published()
        .unwrap();

    assert_eq!(text(&reader, note).await, "after");
    let fresh = harness.second(&reader).await;
    let old = fresh.open_commit::<TextContent>(before).await.unwrap();
    assert_eq!(TextBlock::text(&old), "before", "history stopped opening");
    assert_eq!(text(&fresh, note).await, "after");

    assert!(leaver.summary(note).await.is_err());
    leaver.refresh_keys(Some(vec![note, team])).await.unwrap();
    assert!(
        !leaver.keys().has(new_key),
        "the removed reader got the new key"
    );
    assert!(
        !leaver.try_fetch(after.hash()).await.unwrap(),
        "the server served the removed reader a commit written after"
    );
    let elsewhere = harness.second(&leaver).await;
    assert!(
        !elsewhere.try_fetch(before.hash()).await.unwrap(),
        "the server still serves the removed reader what it could read before"
    );

    harness.stop().await;
}
