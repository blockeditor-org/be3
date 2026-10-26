use super::*;

use be_graph::Access;
use text_block_types::{TextBlock, TextContent};

#[tokio::test]
async fn leaving_a_shared_workspace_leaves_its_rotation_to_who_can_edit_it() {
    let harness = Harness::start().await;
    let owner = harness.owner("owner@example.com").await;
    let guest = harness.member(&owner, "guest@example.com").await;
    let team = workspace(&owner, "Team").await;
    let note = owner
        .create::<TextContent>(BlockParent::Block(team))
        .await
        .unwrap();
    owner
        .save(note, &TextBlock::of("hello"), None)
        .await
        .unwrap();
    let public = owner.keys().contact(guest.account()).unwrap();
    owner
        .share(team, guest.account(), Access::View, &public)
        .await
        .unwrap();
    assert_eq!(text(&guest, note).await, "hello");

    guest.leave(team).await.unwrap();
    assert!(guest.summary(team).await.is_err());
    assert!(guest.key_work().await.unwrap().rotations.is_empty());
    assert_eq!(owner.key_work().await.unwrap().rotations, [team]);
    owner.tend().await.unwrap();
    assert!(owner.key_work().await.unwrap().rotations.is_empty());
    assert_eq!(text(&harness.second(&owner).await, note).await, "hello");

    harness.stop().await;
}
