use be_type_text::TextContent;

use super::*;

/// A block the server refused to make keeps its summary for its unsent row only: listed
/// offline, its folder does not show it, even after something more was typed into it.
#[tokio::test]
async fn a_block_the_server_refused_to_make_is_not_listed_offline() {
    let harness = Harness::start().await;
    let owner = harness.owner("owner@example.com").await;
    let refs = refs::RefsDb::memory();
    let store = MemoryStore::new();
    let laptop = harness.device(&owner, &refs, &store).await;
    let team = workspace(&laptop, "Team").await;
    laptop.children(BlockParent::Block(team)).await.unwrap();
    let refused = Uuid::now_v7();
    laptop
        .keep_creation(
            refused,
            TextContent::CONTENT_TYPE,
            BlockParent::Block(team),
            &be_block_content::BlockMetadata::named("n".repeat(70 * 1024)),
            false,
        )
        .await
        .unwrap();
    laptop.drain(|_| false).await.unwrap();
    assert!(laptop.is_unsent(refused));

    // Typed into the refused block after: it is never sent, and joins it among the unsent, still
    // refused, not taken for lost access.
    laptop
        .save(refused, &be_type_text::TextBlock::of("typed after"), None)
        .await
        .unwrap();
    laptop.drain(|_| false).await.unwrap();
    assert_eq!(
        laptop.refusal(refused),
        Some(be_block_protocol::ErrorCode::InvalidRequest)
    );
    assert_eq!(laptop.pending().operations, 0);
    drop(laptop);

    let offline = harness.offline(&owner, &refs, &store);
    let listed = offline.children(BlockParent::Block(team)).await.unwrap();
    assert!(
        listed.iter().all(|summary| summary.id != refused),
        "the refused block came back into its folder offline"
    );

    harness.stop().await;
}
