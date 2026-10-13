use super::*;

use be_block::{TextBlock, TextContent};

#[tokio::test]
async fn two_peers_converge_through_the_session_owner() {
    let harness = Harness::start().await;
    let first = harness.owner("first@example.com").await;
    let block = first
        .create::<TextContent>(BlockParent::Root)
        .await
        .unwrap();
    first
        .save(block, &TextBlock::of("hello world"), None)
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
    assert_eq!(owner.content().to_text(), "hello world");
    assert_eq!(follower.content().to_text(), "hello world");

    type_at(&mut owner, 0, ">> ").await;
    type_at(&mut follower, 11, "!").await;
    settle(&mut [&mut owner, &mut follower]).await;

    assert_eq!(
        owner.content().to_text(),
        ">> hello world!",
        "the owner did not absorb the follower's concurrent edit"
    );
    assert_eq!(
        follower.content().to_text(),
        owner.content().to_text(),
        "the peers did not converge"
    );

    erase(&mut follower, 3..8).await;
    type_at(&mut follower, 3, "goodbye").await;
    settle(&mut [&mut owner, &mut follower]).await;
    assert_eq!(follower.content().to_text(), ">> goodbye world!");
    assert_eq!(owner.content().to_text(), follower.content().to_text());

    let saved = owner.seal().await.unwrap();
    let head = saved.published().expect("the owner sealed a commit");
    assert!(owner.is_clean());
    assert_eq!(
        owner_peer
            .open::<TextContent>(block)
            .await
            .unwrap()
            .unwrap()
            .to_text(),
        ">> goodbye world!"
    );
    assert_eq!(owner.head(), Some(head));

    harness.stop().await;
}
