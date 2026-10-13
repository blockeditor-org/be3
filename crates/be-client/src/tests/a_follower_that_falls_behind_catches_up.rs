use super::*;

use be_block::{TextBlock, TextContent};

#[tokio::test]
async fn a_follower_that_falls_behind_catches_up() {
    let harness = Harness::start().await;
    let first = harness.owner("first@example.com").await;
    let block = first
        .create::<TextContent>(BlockParent::Root)
        .await
        .unwrap();
    first.save(block, &TextBlock::of(""), None).await.unwrap();

    let owner_peer = Arc::new(first);
    let follower_peer = harness.shared(&owner_peer).await;
    let mut owner = Live::<_, TextContent>::join(Arc::clone(&owner_peer), block)
        .await
        .unwrap();
    let mut follower = Live::<_, TextContent>::join(Arc::clone(&follower_peer), block)
        .await
        .unwrap();
    settle(&mut [&mut owner, &mut follower]).await;

    for index in 0..400 {
        type_at(&mut owner, index, if index % 2 == 0 { "a" } else { "b" }).await;
    }
    settle(&mut [&mut owner, &mut follower]).await;

    assert_eq!(owner.content().to_text().len(), 400);
    assert_eq!(
        follower.content().to_text(),
        owner.content().to_text(),
        "the follower did not recover the edits it fell behind on"
    );

    harness.stop().await;
}
