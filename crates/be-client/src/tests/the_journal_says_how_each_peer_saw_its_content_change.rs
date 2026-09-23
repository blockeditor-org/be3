use super::*;

use be_block::{Counter, CounterContent};

use crate::Journaled;

#[tokio::test]
async fn the_journal_says_how_each_peer_saw_its_content_change() {
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
    let mut on_laptop = Live::<_, CounterContent>::join(Arc::clone(&laptop), block)
        .await
        .unwrap();
    until(&mut [&mut on_phone, &mut on_laptop], "joined", |_| true).await;
    on_phone.take_journal();
    on_laptop.take_journal();

    on_phone.edit(Counter::add(1)).await.unwrap();
    until(
        &mut [&mut on_phone, &mut on_laptop],
        "shared the add",
        |sessions| sessions[1].content().root().value() == 1,
    )
    .await;
    assert_eq!(
        on_phone.take_journal(),
        [Journaled::Edited(Counter::add(1))]
    );
    assert_eq!(
        on_laptop.take_journal(),
        [Journaled::Applied(Counter::add(1))]
    );

    on_laptop.edit(Counter::add(10)).await.unwrap();
    until(
        &mut [&mut on_phone, &mut on_laptop],
        "sequenced the laptop's add",
        |sessions| sessions[0].content().root().value() == 11 && sessions[1].is_clean(),
    )
    .await;
    assert_eq!(
        on_phone.take_journal(),
        [Journaled::Applied(Counter::add(10))]
    );
    assert_eq!(
        on_laptop.take_journal(),
        [Journaled::Edited(Counter::add(10))],
        "the laptop's own edit coming back changed nothing it could see"
    );

    harness.stop().await;
}
