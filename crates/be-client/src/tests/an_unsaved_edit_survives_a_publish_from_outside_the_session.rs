use super::*;

use be_block::{Counter, CounterContent};

#[tokio::test]
async fn an_unsaved_edit_survives_a_publish_from_outside_the_session() {
    let harness = Harness::start().await;
    let phone = Arc::new(harness.owner("phone@example.com").await);
    let block = phone
        .create::<CounterContent>(BlockParent::Root)
        .await
        .unwrap();
    let laptop = harness.shared(&phone).await;
    let offline = harness.shared(&phone).await;
    let mut published = CounterContent::default();
    published.apply(&Counter::add(50));
    let mut on_phone = Live::<_, CounterContent>::join(Arc::clone(&phone), block)
        .await
        .unwrap();
    let mut on_laptop = Live::<_, CounterContent>::join(Arc::clone(&laptop), block)
        .await
        .unwrap();

    on_phone.edit(Counter::add(1)).await.unwrap();
    until(
        &mut [&mut on_phone, &mut on_laptop],
        "shared the add",
        |sessions| sessions[1].content().root().value() == 1,
    )
    .await;
    offline
        .save(block, &published, None)
        .await
        .unwrap()
        .published()
        .unwrap();

    assert!(on_phone.seal().await.unwrap().published().is_none());
    assert!(on_phone.reconcile().await.unwrap().is_clean());

    assert_eq!(on_phone.content().root().value(), 51);
    assert!(on_phone.is_clean());
    until(
        &mut [&mut on_phone, &mut on_laptop],
        "reloaded the merge",
        |sessions| sessions[1].content().root().value() == 51,
    )
    .await;
    assert_eq!(
        offline
            .open::<CounterContent>(block)
            .await
            .unwrap()
            .unwrap()
            .root()
            .value(),
        51
    );

    harness.stop().await;
}
