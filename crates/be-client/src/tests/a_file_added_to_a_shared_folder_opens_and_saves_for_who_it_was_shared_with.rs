use super::*;
use workspace_ui_types::FolderContent;

use be_graph::Access;
use text_block_types::{TextBlock, TextContent};

#[tokio::test]
async fn a_file_added_to_a_shared_folder_opens_and_saves_for_who_it_was_shared_with() {
    let harness = Harness::start().await;
    let owner = harness.owner("owner@example.com").await;
    let guest = harness.member(&owner, "guest@example.com").await;
    let mine = workspace(&owner, "Mine").await;
    let folder = owner
        .create::<FolderContent>(BlockParent::Block(mine))
        .await
        .unwrap();
    let public = owner.keys().contact(guest.account()).unwrap();
    owner
        .share(folder, guest.account(), Access::Edit, &public)
        .await
        .unwrap();

    let added = owner
        .create::<TextContent>(BlockParent::Block(folder))
        .await
        .unwrap();
    owner
        .save(added, &TextBlock::of("added after the share"), None)
        .await
        .unwrap();

    let listed = guest.children(BlockParent::Block(folder)).await.unwrap();
    let seen = listed
        .iter()
        .find(|block| block.id == added)
        .expect("the guest is told of the file added to the folder");
    assert_eq!(seen.access, Access::Edit);
    assert_eq!(seen.parent, Some(BlockParent::Block(folder)));
    assert_eq!(text(&guest, added).await, "added after the share");

    let head = guest.summary(added).await.unwrap().head;
    guest
        .save(
            added,
            &TextBlock::of("added after the share, and edited"),
            head,
        )
        .await
        .unwrap()
        .published()
        .expect("the guest's edit is published");
    assert_eq!(
        text(&harness.second(&owner).await, added).await,
        "added after the share, and edited"
    );

    harness.stop().await;
}
