use be_block_protocol::ErrorCode;
use be_type_text::{TextBlock, TextContent};

use super::*;

/// A name too large for the server: it refuses the metadata (`InvalidRequest`).
fn oversized() -> be_block::BlockMetadata {
    be_block::BlockMetadata::named("n".repeat(70 * 1024))
}

#[tokio::test]
async fn a_creation_the_server_refuses_is_set_aside_and_nothing_waits_behind_it() {
    let harness = Harness::start().await;
    let owner = harness.owner("owner@example.com").await;
    let block = Uuid::now_v7();
    owner
        .keep_creation(
            block,
            TextContent::CONTENT_TYPE,
            BlockParent::Root,
            &oversized(),
            false,
        )
        .await
        .expect("the block is kept here");
    owner
        .save(block, &TextBlock::of("kept"), None)
        .await
        .expect("an edit waits behind the creation");

    let drained = owner.drain(|_| false).await.unwrap();
    assert!(
        drained.contains(&Drained::SetAside {
            block,
            code: ErrorCode::InvalidRequest
        }),
        "{drained:?}"
    );
    assert!(
        !owner.creating(block),
        "the refused creation no longer waits"
    );
    assert_eq!(
        owner.pending().operations,
        0,
        "nothing of the block waits behind it"
    );
    assert!(
        owner.unsent().iter().any(|unsent| unsent.block == block),
        "what was made of it is set aside, to be restored or discarded"
    );

    harness.stop().await;
}
