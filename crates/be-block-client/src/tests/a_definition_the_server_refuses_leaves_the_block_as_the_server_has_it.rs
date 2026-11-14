use be_block_protocol::ErrorCode;
use be_type_text::TextContent;

use super::*;

#[tokio::test]
async fn a_definition_the_server_refuses_leaves_the_block_as_the_server_has_it() {
    let harness = Harness::start().await;
    let owner = harness.owner("owner@example.com").await;
    let block = owner
        .create_block(
            Uuid::now_v7(),
            TextContent::CONTENT_TYPE,
            BlockParent::Root,
            &be_block_content::BlockMetadata::named("Plan"),
            false,
        )
        .await
        .unwrap()
        .id;
    let name = |owner: &Peer<MemoryStore>| {
        owner
            .known_summary(block)
            .and_then(|summary| owner.metadata(&summary).ok())
            .and_then(|metadata| metadata.name())
    };
    owner
        .keep_definition(block, |metadata| {
            metadata.insert(&be_block_content::NAME, &"n".repeat(70 * 1024));
        })
        .await
        .expect("the definition is kept here");
    assert_ne!(name(&owner).as_deref(), Some("Plan"), "it shows at once");

    let drained = owner.drain(|_| false).await.unwrap();
    assert!(
        drained.contains(&Drained::Refused {
            block,
            code: ErrorCode::InvalidRequest,
            restored: true
        }),
        "{drained:?}"
    );
    assert_eq!(
        name(&owner).as_deref(),
        Some("Plan"),
        "the refused name is not shown as if it held"
    );

    harness.stop().await;
}
