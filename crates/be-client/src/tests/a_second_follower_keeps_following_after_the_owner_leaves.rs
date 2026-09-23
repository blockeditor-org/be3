use super::*;

use be_block::{Counter, CounterContent};

#[tokio::test]
async fn a_second_follower_keeps_following_after_the_owner_leaves() {
    let harness = Harness::start().await;
    let phone = Arc::new(harness.owner("phone@example.com").await);
    let block = phone
        .create::<CounterContent>(BlockParent::Root)
        .await
        .unwrap();
    let laptop = harness.shared(&phone).await;
    let tablet = harness.shared(&phone).await;
    let mut on_phone = Live::<_, CounterContent>::join(Arc::clone(&phone), block)
        .await
        .unwrap();
    let mut on_laptop = Live::<_, CounterContent>::join(Arc::clone(&laptop), block)
        .await
        .unwrap();
    let mut on_tablet = Live::<_, CounterContent>::join(Arc::clone(&tablet), block)
        .await
        .unwrap();

    on_phone.edit(Counter::add(1)).await.unwrap();
    on_tablet.edit(Counter::add(2)).await.unwrap();
    until(
        &mut [&mut on_phone, &mut on_laptop, &mut on_tablet],
        "agreed on the first adds",
        |sessions| {
            sessions
                .iter()
                .all(|session| session.content().root().value() == 3)
                && sessions[1..].iter().all(|session| session.is_clean())
        },
    )
    .await;

    phone.leave_session(block).await.unwrap();
    until(
        &mut [&mut on_laptop, &mut on_tablet],
        "handed the session to the laptop",
        |sessions| sessions[0].is_owner() && !sessions[1].is_owner(),
    )
    .await;
    on_tablet.edit(Counter::add(20)).await.unwrap();
    on_laptop.edit(Counter::add(300)).await.unwrap();

    until(
        &mut [&mut on_laptop, &mut on_tablet],
        "converged under the new owner",
        |sessions| {
            sessions
                .iter()
                .all(|session| session.content().root().value() == 323)
                && sessions[1].is_clean()
        },
    )
    .await;
    on_laptop.seal().await.unwrap().published().unwrap();
    assert_eq!(
        tablet
            .open::<CounterContent>(block)
            .await
            .unwrap()
            .unwrap()
            .root()
            .value(),
        323
    );

    harness.stop().await;
}
