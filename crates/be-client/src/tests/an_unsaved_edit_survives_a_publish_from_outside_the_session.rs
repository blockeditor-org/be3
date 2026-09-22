use super::*;

use be_block::{CounterContent, CounterOp};

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
    let mut on_phone = Live::<_, CounterContent>::join(Arc::clone(&phone), block)
        .await
        .unwrap();
    let mut on_laptop = Live::<_, CounterContent>::join(Arc::clone(&laptop), block)
        .await
        .unwrap();

    on_phone.edit(CounterOp::Add { by: 1 }).await.unwrap();
    until(
        &mut [&mut on_phone, &mut on_laptop],
        "shared the add",
        |sessions| sessions[1].content().count() == 1,
    )
    .await;
    offline
        .save(block, &CounterContent::new(50), None)
        .await
        .unwrap()
        .published()
        .unwrap();

    assert!(on_phone.seal().await.unwrap().published().is_none());
    assert!(on_phone.reconcile().await.unwrap().is_clean());

    assert_eq!(on_phone.content().count(), 51);
    assert!(on_phone.is_clean());
    until(
        &mut [&mut on_phone, &mut on_laptop],
        "reloaded the merge",
        |sessions| sessions[1].content().count() == 51,
    )
    .await;
    assert_eq!(
        offline
            .open::<CounterContent>(block)
            .await
            .unwrap()
            .unwrap()
            .count(),
        51
    );

    harness.stop().await;
}
