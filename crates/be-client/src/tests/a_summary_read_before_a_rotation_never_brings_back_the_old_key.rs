use super::*;

use be_graph::Access;
use text_block_types::{TextBlock, TextContent};

#[tokio::test]
async fn a_summary_read_before_a_rotation_never_brings_back_the_old_key() {
    let harness = Harness::start().await;
    let owner = harness.owner("owner@example.com").await;
    let leaver = harness.member(&owner, "leaver@example.com").await;
    let team = workspace(&owner, "Team").await;
    let note = owner
        .create::<TextContent>(BlockParent::Block(team))
        .await
        .unwrap();
    owner
        .save(note, &TextBlock::of("before"), None)
        .await
        .unwrap();
    let public = owner
        .keys()
        .contact(leaver.account())
        .expect("the invitation made them contacts");
    owner
        .share(team, leaver.account(), Access::Edit, &public)
        .await
        .unwrap();
    let stale = owner.summary(note).await.unwrap();
    let old_key = stale.key.unwrap();

    owner
        .grant(team, leaver.account(), Access::None)
        .await
        .unwrap();
    owner.tend().await.unwrap();
    let new_key = owner.summary(note).await.unwrap().key.unwrap();
    assert_ne!(new_key, old_key);

    owner.note(&stale);
    assert_eq!(
        owner.sealing_key(note).await.unwrap(),
        new_key,
        "a summary read before the rotation put the old key back"
    );

    harness.stop().await;
}
