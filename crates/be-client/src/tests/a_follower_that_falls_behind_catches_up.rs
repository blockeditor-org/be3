use super::*;

use be_block::{TextContent, TextOp};

#[tokio::test]
async fn a_follower_that_falls_behind_catches_up() {
    let harness = Harness::start().await;
    let first = harness.owner("first@example.com").await;
    let block = first
        .create::<TextContent>(BlockParent::Root)
        .await
        .unwrap();
    first
        .save(block, &TextContent::from(""), None)
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

    for index in 0..400 {
        owner
            .edit(TextOp::insert(
                index,
                if index % 2 == 0 { "a" } else { "b" },
            ))
            .await
            .unwrap();
    }
    settle(&mut [&mut owner, &mut follower]).await;

    assert_eq!(owner.content().text().len(), 400);
    assert_eq!(
        follower.content().text(),
        owner.content().text(),
        "the follower did not recover the edits it fell behind on"
    );

    harness.stop().await;
}
