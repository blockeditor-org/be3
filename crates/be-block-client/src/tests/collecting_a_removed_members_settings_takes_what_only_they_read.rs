use super::*;

use be_block_graph::SETTINGS_TYPE;
use be_type_text::{TextBlock, TextContent};

#[tokio::test]
async fn collecting_a_removed_members_settings_takes_what_only_they_read() {
    let mut harness = Harness::start().await;
    let admin = harness.owner("admin@example.com").await;
    let member = harness.member(&admin, "member@example.com").await;
    let settings = member
        .create_block(
            Uuid::now_v7(),
            SETTINGS_TYPE,
            BlockParent::Root,
            &be_block_content::BlockMetadata::default(),
            false,
        )
        .await
        .unwrap()
        .id;
    let secret = member
        .create_block(
            Uuid::now_v7(),
            TextContent::CONTENT_TYPE,
            BlockParent::Block(settings),
            &be_block_content::BlockMetadata::default(),
            true,
        )
        .await
        .unwrap()
        .id;
    member
        .save(secret, &TextBlock::of("a key for elsewhere"), None)
        .await
        .unwrap();

    admin.remove_member(member.account()).await.unwrap();
    admin
        .set_parent(settings, BlockParent::Detached)
        .await
        .unwrap();
    let collected = admin.collect_detached().await.unwrap();
    assert!(collected.contains(&settings));
    assert!(
        collected.contains(&secret),
        "the owner-only block inside went with the settings that held it"
    );

    harness.server.restart().await;
    harness.url = format!("ws://{}", harness.server.address);
    let reopened = harness.second(&admin).await;
    assert!(reopened.summary(settings).await.is_err());
    assert!(reopened.summary(secret).await.is_err());

    harness.stop().await;
}
