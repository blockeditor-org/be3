use super::*;

use be_block::{Counter, CounterContent, NAME};

#[tokio::test]
async fn a_rename_and_an_unsaved_edit_both_reach_the_head() {
    let harness = Harness::start().await;
    let phone = Arc::new(harness.owner("phone@example.com").await);
    let block = phone
        .create::<CounterContent>(BlockParent::Root)
        .await
        .unwrap();
    let laptop = harness.shared(&phone).await;
    let mut on_phone = Live::<_, CounterContent>::join(Arc::clone(&phone), block)
        .await
        .unwrap();
    on_phone.edit(Counter::add(1)).await.unwrap();
    on_phone.seal().await.unwrap().published().unwrap();
    on_phone.edit(Counter::add(1)).await.unwrap();

    let renamed = laptop
        .define(block, |metadata| {
            metadata.insert(&NAME, &"Groceries".to_owned());
        })
        .await
        .unwrap();
    assert_ne!(
        renamed.head,
        on_phone.head(),
        "a rename is published as a commit"
    );

    assert!(on_phone.seal().await.unwrap().published().is_none());
    assert!(on_phone.reconcile().await.unwrap().is_clean());

    let summary = laptop.summary(block).await.unwrap();
    assert_eq!(
        laptop.metadata(&summary).unwrap().name().as_deref(),
        Some("Groceries")
    );
    assert_eq!(
        laptop
            .open::<CounterContent>(block)
            .await
            .unwrap()
            .unwrap()
            .root()
            .value(),
        2
    );

    harness.stop().await;
}
