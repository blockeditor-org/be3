use super::*;

use be_block::{TextBlock, TextContent};

#[tokio::test]
async fn a_follower_keeps_its_unconfirmed_typing_through_a_reload() {
    let harness = Harness::start().await;
    let first = harness.owner("first@example.com").await;
    let block = first
        .create::<TextContent>(BlockParent::Root)
        .await
        .unwrap();
    first
        .save(block, &TextBlock::of("start\n"), None)
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

    type_at(&mut follower, 0, "mine ").await;
    let pasted = "a line of pasted text\n".repeat(60_000);
    type_at(&mut owner, 6, &pasted).await;
    let expected = format!("mine start\n{pasted}");
    until(&mut [&mut owner, &mut follower], "converged", |sessions| {
        sessions
            .iter()
            .all(|session| session.content().to_text() == expected)
            && sessions[1].is_clean()
    })
    .await;

    assert!(!follower.is_incompatible());
    harness.stop().await;
}
