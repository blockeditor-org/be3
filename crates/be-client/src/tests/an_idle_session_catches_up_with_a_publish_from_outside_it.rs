use super::*;

use be_block::{Counter, CounterContent};

#[tokio::test]
async fn an_idle_session_catches_up_with_a_publish_from_outside_it() {
    let harness = Harness::start().await;
    let phone = Arc::new(harness.owner("idle@example.com").await);
    let block = phone
        .create::<CounterContent>(BlockParent::Root)
        .await
        .unwrap();
    let laptop = harness.shared(&phone).await;
    let outside = harness.shared(&phone).await;
    let mut on_phone = Live::<_, CounterContent>::join(Arc::clone(&phone), block)
        .await
        .unwrap();
    let mut on_laptop = Live::<_, CounterContent>::join(Arc::clone(&laptop), block)
        .await
        .unwrap();
    assert!(on_phone.is_owner());

    let mut published = CounterContent::default();
    published.apply(&Counter::add(50));
    outside
        .save(block, &published, None)
        .await
        .unwrap()
        .published()
        .unwrap();

    let reached = tokio::time::timeout(PATIENCE, async {
        loop {
            for session in [&mut on_phone, &mut on_laptop] {
                session.poll().await.unwrap();
                session.catch_up().await.unwrap();
            }
            if [&on_phone, &on_laptop]
                .iter()
                .all(|session| session.content().root().value() == 50)
            {
                return;
            }
            let waits = [&mut on_phone, &mut on_laptop]
                .into_iter()
                .map(|session| Box::pin(session.wait()));
            futures_util::future::select_all(waits).await.0.unwrap();
        }
    })
    .await;

    assert!(
        reached.is_ok(),
        "the sessions never showed what was published outside them: {} and {}",
        on_phone.content().root().value(),
        on_laptop.content().root().value()
    );
    harness.stop().await;
}
