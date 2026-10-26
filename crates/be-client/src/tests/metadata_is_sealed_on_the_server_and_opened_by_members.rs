use super::*;

use be_block::NAME;
use counter_types::CounterContent;

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
        .define(block, |metadata| {
            metadata.insert(&NAME, &"Groceries".to_owned())
        })
        .await
        .unwrap();
    assert!(
        !named
            .metadata
            .windows(b"Groceries".len())
            .any(|window| window == b"Groceries"),
        "the server never sees the name"
    );

    let summary = laptop.summary(block).await.unwrap();
    assert_eq!(
        laptop
            .metadata_of_summary(&summary)
            .await
            .unwrap()
            .name()
            .as_deref(),
        Some("Groceries")
    );

    harness.stop().await;
}
