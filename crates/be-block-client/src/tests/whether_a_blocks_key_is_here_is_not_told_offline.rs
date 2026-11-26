use super::*;

use std::time::Duration;

use be_block_graph::Access;

use crate::Event;

/// `Peer::holds_key` answers whether the key a block is sealed with now is here, asking the
/// server for its keys when it is not. Offline it cannot be told: the answer is unreachable,
/// never false, so the host neither shows the block locked nor sets its kept edits aside.
#[tokio::test]
async fn whether_a_blocks_key_is_here_is_not_told_offline() {
    let harness = Harness::start().await;
    let owner = harness.owner("owner@example.com").await;
    let guest = harness.member(&owner, "guest@example.com").await;
    let public = owner.keys().contact(guest.account()).unwrap();
    let mut listed = Vec::new();
    for name in ["Roadmap", "Plans"] {
        let block = workspace(&owner, name).await;
        owner
            .grant(block, guest.account(), Access::Edit)
            .await
            .unwrap();
        let summaries = vec![guest.summary(block).await.unwrap()];
        guest.learn(&summaries).await.unwrap();
        listed.push(block);
    }
    let [roadmap, plans] = listed[..] else {
        unreachable!()
    };

    // The keys answered, with none of the block's.
    assert!(!guest.holds_key(roadmap).await.unwrap());

    let mut events = guest.connection().subscribe();
    owner
        .seal_share(roadmap, guest.account(), &public)
        .await
        .unwrap();
    tokio::time::timeout(Duration::from_secs(60), async {
        while !matches!(events.recv().await.unwrap(), Event::KeysChanged) {}
    })
    .await
    .expect("the server never told the guest of the key sealed to it");
    guest.keys_changed();
    assert!(guest.holds_key(roadmap).await.unwrap());

    // Offline: a key held is told, one not held cannot be.
    guest.detach();
    assert!(guest.holds_key(roadmap).await.unwrap());
    match guest.holds_key(plans).await {
        Err(error) => assert!(error.is_unreachable(), "{error:?}"),
        Ok(holds) => panic!("told offline whether the key is here: {holds}"),
    }

    harness.stop().await;
}
