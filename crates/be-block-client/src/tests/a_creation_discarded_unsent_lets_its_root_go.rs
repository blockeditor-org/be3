use be_block_protocol::ErrorCode;
use be_type_text::TextContent;

use super::*;

/// A block made here whose creation the server refused, set aside and then discarded, never
/// reached a server: the root and log made with it go too, so a block can be made under its id
/// again rather than taken for one that is there.
#[tokio::test]
async fn a_creation_discarded_unsent_lets_its_root_go() {
    let harness = Harness::start().await;
    let owner = harness.owner("owner@example.com").await;
    let block = Uuid::now_v7();
    owner
        .keep_creation(
            block,
            TextContent::CONTENT_TYPE,
            BlockParent::Root,
            // A name too large for the server: it refuses the metadata (`InvalidRequest`).
            &be_block_content::BlockMetadata::named("n".repeat(70 * 1024)),
            false,
        )
        .await
        .expect("the block is kept here");
    assert!(owner.keys().holds_log(block));
    let drained = owner.drain(|_| false).await.unwrap();
    assert!(
        drained.contains(&Drained::SetAside {
            block,
            why: crate::Aside::Refused(ErrorCode::InvalidRequest)
        }),
        "{drained:?}"
    );

    assert!(owner.discard_unsent(block).unwrap());
    assert!(
        !owner.keys().holds_log(block),
        "the discarded creation's root and log are gone"
    );
    owner
        .create_block(
            block,
            TextContent::CONTENT_TYPE,
            BlockParent::Root,
            &be_block_content::BlockMetadata::named("made again"),
            false,
        )
        .await
        .expect("the block is made under its id again");
    assert!(owner.keys().holds_log(block));

    harness.stop().await;
}
