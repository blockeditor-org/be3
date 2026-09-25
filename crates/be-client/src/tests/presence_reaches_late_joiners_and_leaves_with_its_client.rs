use super::*;

use be_block::CounterContent;

const CURSOR: Uuid = Uuid::from_u128(0xc0);

#[tokio::test]
async fn presence_reaches_late_joiners_and_leaves_with_its_client() {
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
    until(
        &mut [&mut on_phone, &mut on_laptop],
        "saw each other join",
        |sessions| {
            sessions
                .iter()
                .all(|session| session.state().participants.len() == 2)
        },
    )
    .await;

    on_phone.set_presence(CURSOR, Some(vec![1])).await.unwrap();
    until(
        &mut [&mut on_phone, &mut on_laptop],
        "showed the phone's cursor to the laptop",
        |sessions| sessions[1].presence().values().any(|value| *value == [1]),
    )
    .await;
    assert!(on_phone.presence().is_empty(), "a peer never sees itself");
    assert!(on_laptop.take_presence_changed());

    let mut on_tablet = Live::<_, CounterContent>::join(Arc::clone(&tablet), block)
        .await
        .unwrap();
    until(
        &mut [&mut on_phone, &mut on_laptop, &mut on_tablet],
        "showed the phone's cursor to the tablet that joined later",
        |sessions| sessions[2].presence().values().any(|value| *value == [1]),
    )
    .await;

    phone.leave_session(block).await.unwrap();
    until(
        &mut [&mut on_laptop, &mut on_tablet],
        "forgot the cursor of the phone that left",
        |sessions| sessions.iter().all(|session| session.presence().is_empty()),
    )
    .await;

    harness.stop().await;
}
