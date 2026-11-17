use super::*;

use be_block_graph::Access;
use be_type_text::{TextBlock, TextContent};
use be_type_workspace::FolderContent;

/// Whether `peer` kept a log of `block` as one it made itself: at its root, naming this account
/// alone, as Manage (`KeyBook::keep_made`).
fn kept_as_made(peer: &Peer<MemoryStore>, block: Uuid) -> bool {
    peer.keys().kept_log(block).is_some_and(|kept| {
        kept.place == 0
            && kept.levels == vec![(peer.account().into_bytes(), be_block_keys::Level::Manage)]
    })
}

/// A block shared with this account, asked for as present with another type (an editor opening
/// it as one, say) or made under its id, is never made here: nothing is pinned or kept for a
/// root that is not the block's, before the device holds its log or after, so the keys its
/// sharer sealed still open, and still do once the device starts again.
#[tokio::test]
async fn making_a_block_whose_log_this_device_holds_keeps_its_root() {
    let harness = Harness::start().await;
    let owner = harness.owner("owner@example.com").await;
    let guest = harness.member(&owner, "guest@example.com").await;
    let mine = workspace(&owner, "Mine").await;
    let block = owner
        .create::<TextContent>(BlockParent::Block(mine))
        .await
        .unwrap();
    owner
        .save(block, &TextBlock::of("shared"), None)
        .await
        .unwrap();
    // Started before the share, so it has read none of the block's keys.
    let refs = refs::RefsDb::memory();
    let store = MemoryStore::new();
    let laptop = harness.device(&guest, &refs, &store).await;
    let public = owner.verified_identity(guest.account()).unwrap();
    owner
        .share(block, guest.account(), Access::Edit, &public)
        .await
        .unwrap();
    assert!(
        !laptop.keys().holds_log(block),
        "the laptop has read nothing of the block yet"
    );

    // Before its keys are read here: the server says it is there, and a creation under its id
    // is refused, leaving nothing behind.
    laptop
        .ensure_type(block, FolderContent::CONTENT_TYPE, BlockParent::Detached)
        .await
        .expect("a block that is there already is not made again");
    assert!(
        !laptop.keys().holds_log(block),
        "nothing was made for it"
    );
    assert!(matches!(
        laptop
            .create_block(
                block,
                TextContent::CONTENT_TYPE,
                BlockParent::Detached,
                &be_block_content::BlockMetadata::default(),
                false,
            )
            .await,
        Err(ClientError::Refused(
            be_block_protocol::ErrorCode::BlockAlreadyExists,
            _
        ))
    ));
    assert!(
        !laptop.keys().holds_log(block),
        "the refused creation let its root and log go"
    );

    assert_eq!(text(&laptop, block).await, "shared");
    let pinned = laptop.keys().root_pin(block);
    assert!(
        matches!(pinned, crate::keys::RootPin::Pinned(_)),
        "reading the shared block pinned its root, not {pinned:?}"
    );
    let kept = laptop.keys().kept_log(block);

    // Once it holds the log: refused here, before anything is pinned or kept.
    laptop
        .ensure_type(block, FolderContent::CONTENT_TYPE, BlockParent::Detached)
        .await
        .expect("a block that is there already is not made again");
    assert!(matches!(
        laptop
            .create_block(
                block,
                TextContent::CONTENT_TYPE,
                BlockParent::Detached,
                &be_block_content::BlockMetadata::default(),
                false,
            )
            .await,
        Err(ClientError::Refused(
            be_block_protocol::ErrorCode::BlockAlreadyExists,
            _
        ))
    ));
    assert_eq!(laptop.keys().root_pin(block), pinned);
    assert_eq!(laptop.keys().kept_log(block), kept);
    assert!(
        !laptop.uncreated(block),
        "no creation of the block waits to be sent"
    );
    drop(laptop);

    let restarted = harness.offline(&guest, &refs, &store);
    assert!(
        !kept_as_made(&restarted, block),
        "the device kept no log of the block as made here"
    );
    assert_eq!(restarted.keys().root_pin(block), pinned);

    harness.stop().await;
}
