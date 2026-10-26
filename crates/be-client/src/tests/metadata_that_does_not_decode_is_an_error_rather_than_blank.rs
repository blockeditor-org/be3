use super::*;

use be_block::{BlockMetadata, NAME};
use counter_types::CounterContent;

#[tokio::test]
async fn metadata_that_does_not_decode_is_an_error_rather_than_blank() {
    let harness = Harness::start().await;
    let phone = harness.owner("phone@example.com").await;
    let block = phone
        .create::<CounterContent>(BlockParent::Root)
        .await
        .unwrap();
    let mut summary = phone
        .define(block, |metadata| {
            metadata.insert(&NAME, &"Groceries".to_owned())
        })
        .await
        .unwrap();

    summary.metadata = phone
        .commits()
        .vault()
        .seal(summary.key.unwrap(), &[0xff, 0xff, 0xff])
        .unwrap();
    assert!(phone.metadata_of_summary(&summary).await.is_err());

    summary.metadata = Vec::new();
    assert_eq!(
        phone.metadata_of_summary(&summary).await.unwrap(),
        BlockMetadata::default()
    );

    harness.stop().await;
}
