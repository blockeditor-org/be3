use super::*;

use be_block::{TextBlock, TextContent};

#[tokio::test]
async fn a_follower_that_falls_behind_after_following_keeps_what_it_had() {
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

    type_at(&mut owner, 0, "start").await;
    until(&mut [&mut owner, &mut follower], "followed", |sessions| {
        sessions[1].content().to_text() == "start"
    })
    .await;

    for index in 0..400 {
        let end = owner.content().to_text().len();
        type_at(&mut owner, end, if index % 2 == 0 { "a" } else { "b" }).await;
    }
    settle(&mut [&mut owner, &mut follower]).await;

    assert!(owner.content().to_text().starts_with("start"));
    assert_eq!(
        follower.content().to_text(),
        owner.content().to_text(),
        "the follower lost what it had applied before it fell behind"
    );

    harness.stop().await;
}
