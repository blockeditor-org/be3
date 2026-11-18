use be_block_graph::Access;
use be_block_protocol::ErrorCode;
use be_type_text::{TextBlock, TextContent};

use super::*;

/// A block shared with this account, opened on a device offline that holds no summary of it
/// (`ensure_type`), is made there and kept in the outbox. Once the device reconnects, the
/// server has the block already, made by someone else: what was made of it here is set aside,
/// its root and log let go, and the block opens at its own root.
#[tokio::test]
async fn a_block_opened_offline_that_someone_else_made_keeps_their_root() {
    let harness = Harness::start().await;
    let owner = harness.owner("owner@example.com").await;
    let guest = harness.member(&owner, "guest@example.com").await;
    let mine = workspace(&owner, "Mine").await;
    let refs = refs::RefsDb::memory();
    let store = MemoryStore::new();
    drop(harness.device(&guest, &refs, &store).await);
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

    let offline = harness.offline(&guest, &refs, &store);
    offline
        .ensure_type(block, TextContent::CONTENT_TYPE, BlockParent::Detached)
        .await
        .expect("offline, the block is made here");
    assert!(offline.creating(block), "its creation waits in the outbox");
    let made = offline.keys().root_pin(block);

    harness.reconnect(&offline, guest.token()).await;
    let drained = offline.drain(|_| false).await.unwrap();
    assert!(
        drained.contains(&Drained::SetAside {
            block,
            why: crate::Aside::Refused(ErrorCode::BlockAlreadyExists)
        }),
        "{drained:?}"
    );
    assert!(
        offline
            .keys()
            .kept_log(block)
            .is_none_or(|kept| kept.place != 0 || Some(kept.head) != pinned_root(made)),
        "the log made here is let go"
    );
    assert_eq!(text(&offline, block).await, "shared");
    let pinned = offline.keys().root_pin(block);
    assert!(
        matches!(pinned, crate::keys::RootPin::Pinned(_)) && pinned != made,
        "the block's own root is pinned, not {pinned:?}"
    );

    // Started again: the keys made with the set-aside creation carry its root, which is not
    // pinned again.
    drop(offline);
    let restarted = harness.offline(&guest, &refs, &store);
    assert_eq!(restarted.keys().root_pin(block), pinned);
    assert!(restarted.keys().holds_log(block));
    harness.reconnect(&restarted, guest.token()).await;
    assert_eq!(text(&restarted, block).await, "shared");
    assert_eq!(restarted.keys().root_pin(block), pinned);

    harness.stop().await;
}

fn pinned_root(pin: crate::keys::RootPin) -> Option<[u8; 32]> {
    match pin {
        crate::keys::RootPin::Pinned(root) => Some(root),
        _ => None,
    }
}
