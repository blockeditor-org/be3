use super::*;

use be_block::{TextBlock, TextContent};

#[tokio::test]
async fn large_pastes_behind_an_unconfirmed_edit_go_out_in_relayable_pieces() {
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
    owner.take_journal();

    type_at(&mut follower, 0, "x").await;
    let paste = "p".repeat(400 * 1024);
    for _ in 0..3 {
        let end = follower.content().to_text().len();
        type_at(&mut follower, end, &paste).await;
    }
    let expected = format!("x{paste}{paste}{paste}");
    until(&mut [&mut owner, &mut follower], "converged", |sessions| {
        sessions
            .iter()
            .all(|session| session.content().to_text() == expected)
            && sessions[1].is_clean()
    })
    .await;

    let received = owner
        .take_journal()
        .into_iter()
        .filter(|entry| matches!(entry, Journaled::Applied(_)))
        .count();
    assert_eq!(
        received, 3,
        "the first letter, two pastes merged, then the third on its own"
    );
    harness.stop().await;
}
