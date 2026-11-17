use be_block_protocol::ErrorCode;
use be_type_text::{TextBlock, TextContent};

use super::*;

/// Two devices of one account make a block under the same id: the server takes the first and
/// refuses the second as there already. Its log starts at the root the second made too, as a
/// root is fixed by the account and the block, so the second keeps that root rather than
/// letting it go, and reads the block, also after it starts again.
#[tokio::test]
async fn a_creation_another_device_of_the_account_made_first_keeps_its_root() {
    let harness = Harness::start().await;
    let account = harness.owner("owner@example.com").await;
    let refs = refs::RefsDb::memory();
    let store = MemoryStore::new();
    let laptop = harness.device(&account, &refs, &store).await;
    let phone = harness.second(&account).await;
    let block = Uuid::now_v7();
    phone
        .create_block(
            block,
            TextContent::CONTENT_TYPE,
            BlockParent::Root,
            &be_block_content::BlockMetadata::named("Made on the phone"),
            false,
        )
        .await
        .expect("the phone makes the block first");
    phone
        .save(block, &TextBlock::of("made on the phone"), None)
        .await
        .unwrap();
    let root = phone.keys().root_pin(block);

    assert!(matches!(
        laptop
            .create_block(
                block,
                TextContent::CONTENT_TYPE,
                BlockParent::Root,
                &be_block_content::BlockMetadata::default(),
                false,
            )
            .await,
        Err(ClientError::Refused(ErrorCode::BlockAlreadyExists, _))
    ));
    assert_eq!(
        laptop.keys().root_pin(block),
        root,
        "the root the server's log starts at is kept"
    );
    assert_eq!(text(&laptop, block).await, "made on the phone");

    drop(laptop);
    let laptop = harness.offline(&account, &refs, &store);
    harness.reconnect(&laptop, account.token()).await;
    assert_eq!(text(&laptop, block).await, "made on the phone");
    assert_eq!(laptop.keys().root_pin(block), root);

    harness.stop().await;
}
