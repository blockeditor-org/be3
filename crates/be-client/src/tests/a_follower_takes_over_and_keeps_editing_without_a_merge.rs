use super::*;

use be_block::{TextContent, TextOp};
use be_commit::MergeResult;

#[tokio::test]
async fn a_follower_takes_over_and_keeps_editing_without_a_merge() {
    let harness = Harness::start().await;
    let phone = harness.owner("phone@example.com").await;
    let block = phone
        .create::<TextContent>(BlockParent::Root)
        .await
        .unwrap();
    phone
        .save(block, &TextContent::from("notes\n"), None)
        .await
        .unwrap();

    let phone = Arc::new(phone);
    let laptop_peer = harness.shared(&phone).await;
    let mut on_phone = Live::<_, TextContent>::join(Arc::clone(&phone), block)
        .await
        .unwrap();
    let mut on_laptop = Live::<_, TextContent>::join(Arc::clone(&laptop_peer), block)
        .await
        .unwrap();
    settle(&mut [&mut on_phone, &mut on_laptop]).await;
    assert!(on_phone.is_owner());
    assert!(!on_laptop.is_owner());

    on_phone
        .edit(TextOp::insert(6, "typed on the phone\n"))
        .await
        .unwrap();
    settle(&mut [&mut on_phone, &mut on_laptop]).await;
    let sealed = on_phone.seal().await.unwrap().published().unwrap();
    assert!(on_phone.is_clean());
    assert_eq!(on_laptop.content().text(), "notes\ntyped on the phone\n");

    phone.leave_session(block).await.unwrap();
    settle(&mut [&mut on_laptop]).await;
    assert!(
        on_laptop.is_owner(),
        "ownership did not move on when the owner left"
    );

    assert!(
        matches!(on_laptop.reconcile().await.unwrap(), MergeResult::Clean(())),
        "taking over a session that was durable at the head demanded a merge"
    );
    assert_eq!(on_laptop.head(), Some(sealed));

    on_laptop
        .edit(TextOp::insert(25, "continued on the laptop\n"))
        .await
        .unwrap();
    let head = on_laptop.seal().await.unwrap().published().unwrap();
    assert_ne!(head, sealed);
    assert_eq!(
        laptop_peer
            .open::<TextContent>(block)
            .await
            .unwrap()
            .unwrap()
            .text(),
        "notes\ntyped on the phone\ncontinued on the laptop\n"
    );

    harness.stop().await;
}
