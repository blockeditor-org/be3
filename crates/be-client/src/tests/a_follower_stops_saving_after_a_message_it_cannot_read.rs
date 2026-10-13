use super::*;

use be_block::{TextBlock, TextContent};

#[tokio::test]
async fn a_follower_stops_saving_after_a_message_it_cannot_read() {
    let harness = Harness::start().await;
    let first = harness.owner("first@example.com").await;
    let block = first
        .create::<TextContent>(BlockParent::Root)
        .await
        .unwrap();
    first
        .save(block, &TextBlock::of("hello"), None)
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
    type_at(&mut owner, 5, " world").await;

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
    let refused = TextBlock::insert(follower.content(), follower.client(), 0, b">> ")
        .expect("there is somewhere to type");
    assert!(follower.edit(refused).await.is_err());
    assert!(follower.seal().await.is_err());

    assert!(!owner.is_diverged(), "the owner's own copy is still right");
    let saved = owner.seal().await.unwrap();
    assert!(
        saved.published().is_some(),
        "the owner could not save its edit"
    );

    harness.stop().await;
}
