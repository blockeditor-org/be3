use be_block_protocol::ErrorCode;
use be_type_text::{TextBlock, TextContent};

use super::*;

/// A block's root is fixed by the account and the block: one device's creation refused and
/// discarded, another device of the account then makes the block under the id, at the same
/// root. The first device pins that root through the other's keys and reads the block, before
/// and after it starts again: only the keys of the discarded creation stop pinning it.
#[tokio::test]
async fn a_block_this_account_makes_on_another_device_after_a_discard_pins_its_root() {
    let harness = Harness::start().await;
    let account = harness.owner("owner@example.com").await;
    let refs = refs::RefsDb::memory();
    let store = MemoryStore::new();
    let laptop = harness.device(&account, &refs, &store).await;
    let block = Uuid::now_v7();
    laptop
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
    let root = laptop.keys().root_pin(block);
    assert!(matches!(root, crate::keys::RootPin::Pinned(_)));
    let drained = laptop.drain(|_| false).await.unwrap();
    assert!(
        drained.contains(&Drained::SetAside {
            block,
            why: crate::Aside::Refused(ErrorCode::InvalidRequest)
        }),
        "{drained:?}"
    );
    assert!(laptop.discard_unsent(block).unwrap());

    let phone = harness.second(&account).await;
    phone
        .create_block(
            block,
            TextContent::CONTENT_TYPE,
            BlockParent::Root,
            &be_block_content::BlockMetadata::named("Made on the phone"),
            false,
        )
        .await
        .expect("the phone makes the block");
    phone
        .save(block, &TextBlock::of("made on the phone"), None)
        .await
        .unwrap();
    assert_eq!(phone.keys().root_pin(block), root, "the same root");

    laptop.refresh_keys(None).await.unwrap();
    assert_eq!(text(&laptop, block).await, "made on the phone");
    assert_eq!(laptop.keys().root_pin(block), root);

    drop(laptop);
    let laptop = harness.offline(&account, &refs, &store);
    harness.reconnect(&laptop, account.token()).await;
    assert_eq!(laptop.keys().root_pin(block), root);
    assert_eq!(text(&laptop, block).await, "made on the phone");
    assert_eq!(laptop.keys().root_pin(block), root);

    harness.stop().await;
}
