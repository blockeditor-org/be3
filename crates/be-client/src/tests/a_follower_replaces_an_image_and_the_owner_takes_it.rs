use super::*;

use be_block::{ImageContent, ImageOp};

#[tokio::test]
async fn a_follower_replaces_an_image_and_the_owner_takes_it() {
    let harness = Harness::start().await;
    let first = harness.owner("first@example.com").await;
    let block = first
        .create::<ImageContent>(BlockParent::Root)
        .await
        .unwrap();
    first
        .save(block, &image("before.png", 1024, 1), None)
        .await
        .unwrap();

    let owner_peer = Arc::new(first);
    let follower_peer = harness.shared(&owner_peer).await;
    let mut owner = Live::<_, ImageContent>::join(Arc::clone(&owner_peer), block)
        .await
        .unwrap();
    let mut follower = Live::<_, ImageContent>::join(Arc::clone(&follower_peer), block)
        .await
        .unwrap();
    settle(&mut [&mut owner, &mut follower]).await;
    assert!(owner.is_owner());

    let replacement = image("after.png", 1024 * 1024, 2);
    assert!(follower.replace(replacement.clone()).await.unwrap());
    assert!(!follower.is_owner());
    until(
        &mut [&mut owner, &mut follower],
        "reloaded the replacement",
        |sessions| sessions[0].content() == &replacement,
    )
    .await;

    let mut renamed = replacement.header().clone();
    renamed.source_name = "renamed.png".into();
    owner.edit(ImageOp::SetHeader(renamed)).await.unwrap();
    until(
        &mut [&mut owner, &mut follower],
        "took the new header",
        |sessions| sessions[1].content().header().source_name == "renamed.png",
    )
    .await;
    assert_eq!(follower.content().data(), replacement.data());

    harness.stop().await;
}
