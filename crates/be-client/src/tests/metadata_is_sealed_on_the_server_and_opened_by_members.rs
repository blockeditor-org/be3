use super::*;

use be_block::{BlockMetadata, CounterContent};

#[tokio::test]
async fn metadata_is_sealed_on_the_server_and_opened_by_members() {
    let harness = Harness::start().await;
    let phone = harness.owner("phone@example.com").await;
    let laptop = harness.second(&phone).await;
    let block = phone
        .create::<CounterContent>(BlockParent::Root)
        .await
        .unwrap();

    let named = phone
        .set_metadata(block, &BlockMetadata::named("Groceries"))
        .await
        .unwrap();
    assert!(
        !named
            .metadata
            .windows(b"Groceries".len())
            .any(|window| window == b"Groceries"),
        "the server never sees the name"
    );

    let listed = laptop.list_blocks().await.unwrap();
    let summary = listed.iter().find(|summary| summary.id == block).unwrap();
    assert_eq!(laptop.metadata(summary).name.as_deref(), Some("Groceries"));

    harness.stop().await;
}
