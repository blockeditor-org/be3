use super::*;

use be_block::{Counter, CounterContent};

#[tokio::test]
async fn a_follower_that_claims_as_it_joins_waits_until_it_has_caught_up() {
    let harness = Harness::lapsing().await;
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
    for _ in 0..3 {
        on_phone.edit(Counter::add(1)).await.unwrap();
    }

    let mut on_laptop = Live::<_, CounterContent>::join(Arc::clone(&laptop), block)
        .await
        .unwrap();
    let mut on_tablet = Live::<_, CounterContent>::join(Arc::clone(&tablet), block)
        .await
        .unwrap();
    assert!(
        !on_laptop.claim().await.unwrap(),
        "claimed before hearing what the owner had not saved"
    );
    let reached = tokio::time::timeout(PATIENCE, async {
        loop {
            for session in [&mut on_phone, &mut on_laptop, &mut on_tablet] {
                session.poll().await.unwrap();
            }
            if on_laptop.claim().await.unwrap() {
                return;
            }
            let mut sessions = [&mut on_phone, &mut on_laptop, &mut on_tablet];
            let waits = sessions.iter_mut().map(|session| Box::pin(session.wait()));
            futures_util::future::select_all(waits).await.0.unwrap();
        }
    })
    .await;
    assert!(reached.is_ok(), "the laptop never took the session");

    on_laptop.edit(Counter::add(10)).await.unwrap();
    on_tablet.edit(Counter::add(100)).await.unwrap();
    until(
        &mut [&mut on_phone, &mut on_laptop, &mut on_tablet],
        "followed the laptop's numbering",
        |sessions| {
            sessions
                .iter()
                .all(|session| session.content().root().value() == 113)
                && !sessions[0].is_owner()
                && sessions[2].is_clean()
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
        113
    );

    harness.stop().await;
}
