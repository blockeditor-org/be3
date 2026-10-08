use super::*;

use be_block::{TextBlock, TextContent};

#[tokio::test]
async fn an_edit_too_large_to_relay_is_saved_as_a_commit() {
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

    let pasted = "a line of pasted text\n".repeat(500_000);
    owner.take_journal();
    type_at(&mut owner, 6, &pasted).await;
    assert_eq!(
        owner.take_journal(),
        [crate::Journaled::Replaced { edits: 1 }],
        "the replacement lost count of the edit it was saved for"
    );
    assert!(
        !owner_peer.connection().is_closed(),
        "relaying the large edit broke the connection"
    );
    let expected = format!("start\n{pasted}");
    until(
        &mut [&mut owner, &mut follower],
        "shared the large edit",
        |sessions| sessions[1].content().to_text().len() == expected.len(),
    )
    .await;
    assert_eq!(follower.content().to_text(), expected);

    type_at(&mut owner, 0, "> ").await;
    until(
        &mut [&mut owner, &mut follower],
        "shared a small edit after it",
        |sessions| sessions[1].content().to_text().starts_with("> start"),
    )
    .await;
    let saved = owner_peer
        .open::<TextContent>(block)
        .await
        .unwrap()
        .unwrap();
    assert!(
        saved.to_text().contains(&pasted),
        "the large edit never reached the server"
    );

    harness.stop().await;
}
