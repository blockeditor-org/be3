use super::*;

use be_block_graph::Access;
use be_type_text::{TextBlock, TextContent};
use be_type_workspace::FolderContent;

/// Making a block under the id of one shared with this account, whose summary is not here as
/// the type asked for (an editor opening it as another type, say), is refused before anything
/// is kept: the root this device pinned for it stays, so the keys its sharer sealed still open.
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
    let public = owner.verified_identity(guest.account()).unwrap();
    owner
        .share(block, guest.account(), Access::Edit, &public)
        .await
        .unwrap();
    assert_eq!(text(&guest, block).await, "shared");
    let pinned = guest.keys().root_pin(block);
    assert!(
        matches!(pinned, crate::keys::RootPin::Pinned(_)),
        "reading the shared block pinned its root, not {pinned:?}"
    );

    guest
        .ensure_type(block, FolderContent::CONTENT_TYPE, BlockParent::Detached)
        .await
        .expect("a block that is there already is not made again");
    assert_eq!(guest.keys().root_pin(block), pinned);
    assert!(
        !guest.uncreated(block),
        "no creation of the block waits to be sent"
    );
    assert_eq!(text(&guest, block).await, "shared");

    harness.stop().await;
}
