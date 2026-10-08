use super::*;

use be_block::{TextBlock, TextContent};

#[tokio::test]
async fn typing_behind_an_unconfirmed_edit_goes_out_as_one_edit() {
    let harness = Harness::start().await;
    let first = harness.owner("first@example.com").await;
    let block = first
        .create::<TextContent>(BlockParent::Root)
        .await
        .unwrap();
    first
        .save(block, &TextBlock::of("base"), None)
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
    until(
        &mut [&mut owner, &mut follower],
        "loaded the note",
        |sessions| {
            sessions
                .iter()
                .all(|session| session.content().to_text() == "base")
        },
    )
    .await;
    owner.take_journal();

    let typed = "abcdefghij";
    for at in 0..typed.len() {
        type_at(&mut follower, 4 + at, &typed[at..=at]).await;
    }
    assert_eq!(follower.content().to_text(), "baseabcdefghij");
    assert!(!follower.is_clean());

    let expected = format!("base{typed}");
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
    assert_eq!(received, 2, "the first letter, then the rest as one edit");
    harness.stop().await;
}
