use super::*;
use workspace_ui_types::FolderContent;

use be_graph::Access;
use text_block_types::{TextBlock, TextContent};

#[tokio::test]
async fn a_folder_shared_with_a_contact_opens_for_them_alone() {
    let harness = Harness::start().await;
    let owner = harness.owner("owner@example.com").await;
    let guest = harness.member(&owner, "guest@example.com").await;
    let mine = workspace(&owner, "Mine").await;
    let folder = owner
        .create::<FolderContent>(BlockParent::Block(mine))
        .await
        .unwrap();
    let inside = owner
        .create::<TextContent>(BlockParent::Block(folder))
        .await
        .unwrap();
    owner
        .save(inside, &TextBlock::of("in the folder"), None)
        .await
        .unwrap();
    let beside = owner
        .create::<TextContent>(BlockParent::Block(mine))
        .await
        .unwrap();
    owner
        .save(beside, &TextBlock::of("beside it"), None)
        .await
        .unwrap();

    let public = owner.keys().contact(guest.account()).unwrap();
    owner
        .share(folder, guest.account(), Access::View, &public)
        .await
        .unwrap();
    assert_eq!(text(&guest, inside).await, "in the folder");
    assert!(guest.summary(beside).await.is_err());
    assert_eq!(
        text(&harness.second(&owner).await, inside).await,
        "in the folder"
    );
    assert_eq!(
        text(&harness.second(&owner).await, beside).await,
        "beside it"
    );

    harness.stop().await;
}
