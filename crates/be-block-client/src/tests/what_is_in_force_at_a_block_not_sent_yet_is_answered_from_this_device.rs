use be_type_text::TextContent;

use super::*;

#[tokio::test]
async fn what_is_in_force_at_a_block_not_sent_yet_is_answered_from_this_device() {
    let harness = Harness::start().await;
    let owner = harness.owner("owner@example.com").await;
    let team = workspace(&owner, "Team").await;
    owner.open_settings(team).await.unwrap();
    let block = Uuid::now_v7();
    owner
        .keep_creation(
            block,
            TextContent::CONTENT_TYPE,
            BlockParent::Block(team),
            &be_block_content::BlockMetadata::named("Plan"),
            false,
        )
        .await
        .expect("the block is kept here");

    owner
        .open_settings(block)
        .await
        .expect("the settings above a block the server does not know yet are this device's");
    owner
        .backrefs(block)
        .await
        .expect("so are the blocks that point at it");
    assert!(
        owner
            .children(BlockParent::Block(team))
            .await
            .unwrap()
            .iter()
            .any(|summary| summary.id == block),
        "its folder lists it before the server has it"
    );

    harness.stop().await;
}
