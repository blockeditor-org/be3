use super::*;
use be_type_workspace::FolderContent;

use be_block_graph::Access;
use be_block_protocol::PushMessage;
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
    let beside = owner
        .create::<TextContent>(BlockParent::Block(folder))
        .await
        .unwrap();
    for block in [inside, beside] {
        owner
            .save(block, &TextBlock::of("in the folder"), None)
            .await
            .unwrap();
    }
    let public = owner.keys().contact(guest.account()).unwrap();
    owner
        .share(folder, guest.account(), Access::View, &public)
        .await
        .unwrap();
    assert_eq!(text(&guest, inside).await, "in the folder");
    assert_eq!(text(&guest, beside).await, "in the folder");

    let sent = 1_000_000;
    let notification = crate::Notification {
        block: inside,
        sender: owner.account(),
        sent,
        text: format!("Lunch? {}", "x".repeat(crate::MAX_NOTIFICATION_TEXT)),
    };
    let sealed = owner.seal_notification(&notification).await.unwrap();
    assert!(
        !sealed.windows(5).any(|window| window == b"Lunch"),
        "a notification travels sealed"
    );
    let pushed = |block: Uuid| PushMessage {
        organization: Uuid::nil(),
        block,
        sender: owner.account(),
        sealed: sealed.clone(),
    };
    let opened = guest.open_notification(&pushed(inside), sent + 1).unwrap();
    assert_eq!(opened.sender, owner.account());
    assert_eq!(opened.sent, sent);
    assert!(opened.text.starts_with("Lunch? "));
    assert_eq!(
        opened.text.len(),
        crate::MAX_NOTIFICATION_TEXT,
        "the text is cut to fit a push"
    );
    assert!(
        guest.open_notification(&pushed(beside), sent + 1).is_err(),
        "a notification sent about one block does not open as another's"
    );
    assert!(
        guest
            .open_notification(
                &PushMessage {
                    sender: guest.account(),
                    ..pushed(inside)
                },
                sent + 1
            )
            .is_err(),
        "a notification one member sealed does not open as another's"
    );
    assert!(
        guest
            .open_notification(
                &pushed(inside),
                sent + crate::MAX_NOTIFICATION_AGE_MILLISECONDS + 1
            )
            .is_err(),
        "a notification resent long after it was sent does not open"
    );
    assert!(
        other.open_notification(&pushed(inside), sent + 1).is_err(),
        "a member the block was not shared with opened its notification"
    );

    harness.stop().await;
}
