use super::*;
use workspace_ui_types::WorkspaceContent;

use be_block::BlockMetadata;
use text_block_types::{TextBlock, TextContent};

#[tokio::test]
async fn two_peers_of_one_account_read_each_others_blocks() {
    let harness = Harness::start().await;
    let phone = harness.owner("phone@example.com").await;
    let workspace = phone
        .create_block(
            Uuid::now_v7(),
            WorkspaceContent::CONTENT_TYPE,
            BlockParent::Root,
            &BlockMetadata::named("My files"),
        )
        .await
        .unwrap()
        .id;
    let note = phone
        .create::<TextContent>(BlockParent::Block(workspace))
        .await
        .unwrap();
    phone
        .save(note, &TextBlock::of("written on the phone"), None)
        .await
        .unwrap();

    let laptop = harness.second(&phone).await;
    let opened = laptop.open::<TextContent>(note).await.unwrap().unwrap();
    assert_eq!(TextBlock::text(&opened), "written on the phone");
    let summary = laptop.summary(workspace).await.unwrap();
    assert_eq!(
        laptop
            .metadata_of_summary(&summary)
            .await
            .unwrap()
            .name()
            .as_deref(),
        Some("My files")
    );

    let reply = laptop
        .create::<TextContent>(BlockParent::Block(workspace))
        .await
        .unwrap();
    laptop
        .save(reply, &TextBlock::of("written on the laptop"), None)
        .await
        .unwrap();
    let read = phone.open::<TextContent>(reply).await.unwrap().unwrap();
    assert_eq!(
        TextBlock::text(&read),
        "written on the laptop",
        "a block made after this peer connected did not open with the account's keys"
    );

    let impostor = Peer::connect(
        PeerConfig::new(
            &harness.url,
            identity("someone who is not the account"),
            Credentials::Token(phone.token().to_owned()),
        )
        .organization(Some(phone.organization())),
        MemoryStore::new(),
    )
    .await
    .unwrap();
    assert!(
        matches!(
            impostor.open::<TextContent>(note).await,
            Err(ClientError::Locked(_))
        ),
        "a peer without the account's identity secret read its blocks"
    );

    harness.stop().await;
}
