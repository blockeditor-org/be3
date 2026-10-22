use super::*;

use be_block::{Counter, CounterContent, Settling};

use crate::Journaled;

#[tokio::test]
async fn a_peer_edit_reaching_a_follower_mid_gesture_goes_beneath_its_own() {
    let harness = Harness::start().await;
    let phone = Arc::new(harness.owner("phone@example.com").await);
    let block = phone
        .create::<CounterContent>(BlockParent::Root)
        .await
        .unwrap();
    let laptop = harness.shared(&phone).await;
    let mut owner = Live::<_, CounterContent>::join(Arc::clone(&phone), block)
        .await
        .unwrap();
    let mut follower = Live::<_, CounterContent>::join(Arc::clone(&laptop), block)
        .await
        .unwrap();
    until(&mut [&mut owner, &mut follower], "joined", |_| true).await;
    follower.take_journal();
    let mut editor = Settling::new(follower.content().clone(), Vec::new());

    for _ in 0..3 {
        follower.edit(Counter::add(1)).await.unwrap();
    }
    owner.edit(Counter::add(10)).await.unwrap();
    until(&mut [&mut owner, &mut follower], "converged", |sessions| {
        sessions
            .iter()
            .all(|session| session.content().root().value() == 13)
            && sessions[1].is_clean()
    })
    .await;

    let journal = follower.take_journal();
    assert!(
        journal
            .iter()
            .any(|entry| matches!(entry, Journaled::Beneath { .. })),
        "the owner's edit goes beneath the follower's unconfirmed ones: {journal:?}"
    );
    assert!(
        !journal
            .iter()
            .any(|entry| matches!(entry, Journaled::Replaced { .. })),
        "the follower never replaces its content for a peer's edit: {journal:?}"
    );
    for entry in journal {
        let landed = entry.landed().expect("nothing was replaced");
        assert!(editor.land(landed), "every entry lands on what came before");
    }
    assert_eq!(editor.visible().root().value(), 13);
    editor.settle_to(0);
    assert_eq!(editor.settled().root().value(), 13);

    harness.stop().await;
}
