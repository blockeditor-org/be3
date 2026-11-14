use be_type_text::TextContent;

use super::*;

/// The server does not know of a move that waits in this device's outbox: a summary read or
/// heard meanwhile shows the block where this device moved it, and keeps it there.
#[tokio::test]
async fn a_summary_read_while_a_move_waits_to_be_sent_shows_the_move() {
    let harness = Harness::start().await;
    let owner = harness.owner("owner@example.com").await;
    let team = workspace(&owner, "Team").await;
    let note = owner
        .create::<TextContent>(BlockParent::Block(team))
        .await
        .unwrap();

    let refs = refs::RefsDb::memory();
    let store = MemoryStore::new();
    let laptop = harness.device(&owner, &refs, &store).await;
    laptop.summary(note).await.unwrap();
    laptop.keep_move(note, BlockParent::Root).unwrap();

    assert_eq!(
        laptop.summary(note).await.unwrap().parent,
        Some(BlockParent::Root),
        "the summary showed where the server still has it"
    );
    assert_eq!(
        laptop.known_summary(note).unwrap().parent,
        Some(BlockParent::Root),
        "the server's answer replaced the move this device keeps"
    );
    // A summary the server sends of its own accord (news of the block) is kept the same way.
    let told = owner.summary(note).await.unwrap();
    assert_eq!(told.parent, Some(BlockParent::Block(team)));
    laptop.learn(std::slice::from_ref(&told)).await.unwrap();
    assert_eq!(
        laptop.known_summary(note).unwrap().parent,
        Some(BlockParent::Root),
        "news of the block from the server undid the move this device keeps"
    );
    assert_eq!(
        laptop.summary_with_outbox(told).parent,
        Some(BlockParent::Root)
    );

    laptop.drain(|_| false).await.unwrap();
    assert_eq!(
        owner.summary(note).await.unwrap().parent,
        Some(BlockParent::Root)
    );
    harness.stop().await;
}
