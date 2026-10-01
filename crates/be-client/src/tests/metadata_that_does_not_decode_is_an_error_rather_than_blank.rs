use super::*;

use be_block::{BlockMetadata, CounterContent};

#[tokio::test]
async fn metadata_that_does_not_decode_is_an_error_rather_than_blank() {
    let harness = Harness::start().await;
    let phone = harness.owner("phone@example.com").await;
    let block = phone
        .create::<CounterContent>(BlockParent::Root)
        .await
        .unwrap();
    let mut summary = phone
        .set_metadata(block, &BlockMetadata::named("Groceries"))
        .await
        .unwrap();

    summary.metadata = phone.commits().vault().seal(&[0xff, 0xff, 0xff]);
    assert!(phone.metadata(&summary).is_err());

    summary.metadata = Vec::new();
    assert_eq!(phone.metadata(&summary).unwrap(), BlockMetadata::default());

    harness.stop().await;
}
