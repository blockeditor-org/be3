use be_block_content::BlockContent;
use be_block_graph::Access;
use be_block_protocol::ErrorCode;
use be_type_text::{TextBlock, TextContent};

use super::*;

/// A block file imported here under the id of a block someone else made and shared with this
/// account is not that block: the server has it already, at another root, so the import is
/// set aside and its root let go, and the shared block opens at its own root, also after the
/// device starts again.
#[tokio::test]
async fn an_import_under_the_id_of_a_block_someone_else_made_is_set_aside() {
    let harness = Harness::start().await;
    let owner = harness.owner("owner@example.com").await;
    let guest = harness.member(&owner, "guest@example.com").await;
    let mine = workspace(&owner, "Mine").await;
    let refs = refs::RefsDb::memory();
    let store = MemoryStore::new();
    let laptop = harness.device(&guest, &refs, &store).await;
    let block = owner
        .create::<TextContent>(BlockParent::Block(mine))
        .await
        .unwrap();
    owner
        .save(block, &TextBlock::of("shared"), None)
        .await
        .unwrap();
    let public = owner.verified_identity(guest.account()).unwrap();
    owner
        .share(block, guest.account(), Access::Edit, &public)
        .await
        .unwrap();

    let answers = import_one(
        &laptop,
        crate::transfer::Saved {
            id: block,
            block_type: TextContent::CONTENT_TYPE,
            parent: None,
            metadata: be_block_content::BlockMetadata::named("Imported"),
            references: Vec::new(),
            content: Some(TextBlock::of("imported").encode()),
            log: Vec::new(),
        },
    )
    .await;
    assert!(
        matches!(
            answers.as_slice(),
            [Err(ClientError::Refused(ErrorCode::BlockAlreadyExists, _))]
        ),
        "{answers:?}"
    );
    assert!(
        laptop.unsent().iter().any(|unsent| unsent.block == block),
        "what was imported is set aside"
    );
    assert_eq!(text(&laptop, block).await, "shared");
    let pinned = laptop.keys().root_pin(block);
    assert!(matches!(pinned, crate::keys::RootPin::Pinned(_)));

    drop(laptop);
    let laptop = harness.offline(&guest, &refs, &store);
    assert_eq!(laptop.keys().root_pin(block), pinned);
    harness.reconnect(&laptop, guest.token()).await;
    assert_eq!(text(&laptop, block).await, "shared");

    harness.stop().await;
}
