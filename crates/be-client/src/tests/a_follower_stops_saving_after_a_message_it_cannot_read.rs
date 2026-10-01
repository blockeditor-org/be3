use super::*;

use be_block::{TextContent, TextOp};

#[tokio::test]
async fn a_follower_stops_saving_after_a_message_it_cannot_read() {
    let harness = Harness::start().await;
    let first = harness.owner("first@example.com").await;
    let block = first
        .create::<TextContent>(BlockParent::Root)
        .await
        .unwrap();
    first
        .save(block, &TextContent::from("hello"), None)
        .await
        .unwrap();

    let owner_peer = Arc::new(first);
    let follower_peer = harness.shared(&owner_peer).await;
    let newer_peer = harness.shared(&owner_peer).await;
    let mut owner = Live::<_, TextContent>::join(Arc::clone(&owner_peer), block)
        .await
        .unwrap();
    let mut follower = Live::<_, TextContent>::join(Arc::clone(&follower_peer), block)
        .await
        .unwrap();
    owner.edit(TextOp::insert(5, " world")).await.unwrap();

    newer_peer
        .relay(block, None, &[0xff, 0xff, 0xff, 0xff])
        .await
        .unwrap();
    until(
        &mut [&mut owner, &mut follower],
        "noticed the unreadable message",
        |sessions| sessions.iter().all(|session| session.is_incompatible()),
    )
    .await;

    assert!(follower.is_diverged());
    assert!(follower.edit(TextOp::insert(0, ">> ")).await.is_err());
    assert!(follower.seal().await.is_err());

    assert!(!owner.is_diverged(), "the owner's own copy is still right");
    let saved = owner.seal().await.unwrap();
    assert!(
        saved.published().is_some(),
        "the owner could not save its edit"
    );

    harness.stop().await;
}
