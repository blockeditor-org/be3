use super::*;

use be_block::{TextContent, TextOp};

#[tokio::test]
async fn two_peers_converge_through_the_session_owner() {
    let harness = Harness::start().await;
    let first = harness.owner("first@example.com").await;
    let block = first
        .create::<TextContent>(BlockParent::Root)
        .await
        .unwrap();
    first
        .save(block, &TextContent::from("hello world"), None)
        .await
        .unwrap();

    let owner_peer = Arc::new(first);
    let follower_peer = harness.shared(&owner_peer).await;
    let mut owner = Live::<_, TextContent>::join(Arc::clone(&owner_peer), block)
        .await
        .unwrap();
    let mut follower = Live::<_, TextContent>::join(Arc::clone(&follower_peer), block)
        .await
        .unwrap();
    settle(&mut [&mut owner, &mut follower]).await;

    assert!(owner.is_owner());
    assert!(!follower.is_owner());
    assert_eq!(owner.content().text(), "hello world");
    assert_eq!(follower.content().text(), "hello world");

    owner.edit(TextOp::insert(0, ">> ")).await.unwrap();
    follower.edit(TextOp::insert(11, "!")).await.unwrap();
    settle(&mut [&mut owner, &mut follower]).await;

    assert_eq!(
        owner.content().text(),
        ">> hello world!",
        "the owner did not absorb the follower's concurrent edit"
    );
    assert_eq!(
        follower.content().text(),
        owner.content().text(),
        "the peers did not converge"
    );

    follower.edit(TextOp::delete(3, 5)).await.unwrap();
    follower.edit(TextOp::insert(3, "goodbye")).await.unwrap();
    settle(&mut [&mut owner, &mut follower]).await;
    assert_eq!(follower.content().text(), ">> goodbye world!");
    assert_eq!(owner.content().text(), follower.content().text());

    let saved = owner.seal().await.unwrap();
    let head = saved.published().expect("the owner sealed a commit");
    assert!(owner.is_clean());
    assert_eq!(
        owner_peer
            .open::<TextContent>(block)
            .await
            .unwrap()
            .unwrap()
            .text(),
        ">> goodbye world!"
    );
    assert_eq!(owner.head(), Some(head));

    harness.stop().await;
}
