use super::*;
use be_type_workspace::FolderContent;

use be_block_graph::Access;
use be_type_text::{TextBlock, TextContent};

#[tokio::test]
async fn a_notification_opens_only_for_the_blocks_members() {
    let harness = Harness::start().await;
    let owner = harness.owner("owner@example.com").await;
    let guest = harness.member(&owner, "guest@example.com").await;
    let other = harness.member(&owner, "other@example.com").await;
    let mine = workspace(&owner, "Mine").await;
    let folder = owner
        .create::<FolderContent>(BlockParent::Block(mine))
        .await
        .unwrap();
    let inside = owner
        .create::<TextContent>(BlockParent::Block(folder))
        .await
        .unwrap();
    owner
        .save(inside, &TextBlock::of("in the folder"), None)
        .await
        .unwrap();
    let public = owner.keys().contact(guest.account()).unwrap();
    owner
        .share(folder, guest.account(), Access::View, &public)
        .await
        .unwrap();
    assert_eq!(text(&guest, inside).await, "in the folder");

    let notification = crate::Notification {
        title: "Plans".to_owned(),
        author: "Owner".to_owned(),
        text: "x".repeat(crate::MAX_NOTIFICATION_TEXT + 10),
    };
    let sealed = owner.seal_notification(inside, &notification).await.unwrap();
    assert!(
        !sealed.windows(5).any(|window| window == b"Plans"),
        "a notification travels sealed"
    );
    let opened = guest.open_notification(&sealed).unwrap();
    assert_eq!(opened.title, "Plans");
    assert_eq!(opened.author, "Owner");
    assert_eq!(
        opened.text.len(),
        crate::MAX_NOTIFICATION_TEXT,
        "the text is cut to fit a push"
    );
    assert!(
        other.open_notification(&sealed).is_err(),
        "a member the block was not shared with opened its notification"
    );

    harness.stop().await;
}
