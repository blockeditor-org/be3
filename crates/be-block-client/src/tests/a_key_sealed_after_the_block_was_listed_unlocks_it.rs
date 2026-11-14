use super::*;

use std::time::Duration;

use be_block_graph::Access;
use be_block_protocol::BlockSummary;

use crate::Event;

/// Grants `guest` a workspace named `name` without sealing its key, and has the guest list it
/// and find it locked.
async fn granted_locked(owner: &Peer<MemoryStore>, guest: &Peer<MemoryStore>, name: &str) -> Uuid {
    // A workspace is a scope root already, so sealing its key later is the one row put.
    let block = workspace(owner, name).await;
    owner
        .grant(block, guest.account(), Access::View)
        .await
        .unwrap();
    let summary = listed(guest, block).await;
    assert!(
        matches!(guest.metadata(&summary), Err(ClientError::Locked(_))),
        "the guest opened {name} before its key was sealed to it"
    );
    block
}

/// The guest's summary of `block`, learned.
async fn listed(guest: &Peer<MemoryStore>, block: Uuid) -> BlockSummary {
    let listed = guest.list(Listing::Block(block)).await.unwrap();
    guest.learn(&listed).await.unwrap();
    let [summary] = listed.as_slice() else {
        panic!("the guest lists {listed:?}");
    };
    summary.clone()
}

/// A share whose key reaches the server after its grant (as `Peer::share` sends them, or a
/// pending grant sealed later): the guest's device lists the block while it has no key, then
/// opens it once the key is sealed to it, though it already looked for that key once. The
/// server tells it so (`keys-changed`); a device away as the key was sealed asks for every key
/// as it attaches again.
#[tokio::test]
async fn a_key_sealed_after_the_block_was_listed_unlocks_it() {
    let harness = Harness::start().await;
    let owner = harness.owner("owner@example.com").await;
    let guest = harness.member(&owner, "guest@example.com").await;
    let public = owner.keys().contact(guest.account()).unwrap();

    let mut events = guest.connection().subscribe();
    let roadmap = granted_locked(&owner, &guest, "Roadmap").await;
    owner
        .seal_share(roadmap, guest.account(), &public)
        .await
        .unwrap();
    // A bound on a signal that comes, not a wait for time to pass: missing, it fails here.
    tokio::time::timeout(Duration::from_secs(60), async {
        while !matches!(events.recv().await.unwrap(), Event::KeysChanged) {}
    })
    .await
    .expect("the server never told the guest of the key sealed to it");
    guest.keys_changed();
    let summary = listed(&guest, roadmap).await;
    assert_eq!(
        guest.metadata(&summary).unwrap().name().as_deref(),
        Some("Roadmap")
    );

    let plans = granted_locked(&owner, &guest, "Plans").await;
    guest.detach();
    owner
        .seal_share(plans, guest.account(), &public)
        .await
        .unwrap();
    harness.reconnect(&guest, guest.token()).await;
    let summary = listed(&guest, plans).await;
    assert_eq!(
        guest.metadata(&summary).unwrap().name().as_deref(),
        Some("Plans"),
        "a device away as the key was sealed did not ask for it again on attaching"
    );

    harness.stop().await;
}
