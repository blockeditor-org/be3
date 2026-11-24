use be_type_text::TextContent;

use super::*;

/// The same block file imported on two devices of one account: the second import's block is
/// there already at the root it made too, so it stands, and nothing of it is let go.
#[tokio::test]
async fn an_import_another_device_of_the_account_made_first_stands() {
    let harness = Harness::start().await;
    let account = harness.owner("owner@example.com").await;
    let refs = refs::RefsDb::memory();
    let store = MemoryStore::new();
    let laptop = harness.device(&account, &refs, &store).await;
    let phone = harness.second(&account).await;
    let saved = crate::transfer::SavedBlock {
        id: Uuid::now_v7(),
        block_type: TextContent::CONTENT_TYPE,
        parent: None,
        metadata: be_block_content::BlockMetadata::named("Imported"),
        references: Vec::new(),
        content: None,
        log: Vec::new(),
    };
    let block = saved.id;
    assert!(
        matches!(import_one(&phone, saved.clone()).await.as_slice(), [Ok(_)]),
        "the phone imports it"
    );
    let root = phone.keys().root_pin(block);

    let answers = import_one(&laptop, saved).await;
    assert!(matches!(answers.as_slice(), [Ok(_)]), "{answers:?}");
    assert!(laptop.unsent().is_empty(), "nothing is set aside");
    assert_eq!(laptop.keys().root_pin(block), root);

    drop(laptop);
    let laptop = harness.offline(&account, &refs, &store);
    assert_eq!(laptop.keys().root_pin(block), root);

    harness.stop().await;
}
