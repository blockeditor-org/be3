use super::*;

use be_block_graph::Access;

use crate::Event;

/// A share whose key reaches the server after its grant (as `Peer::share` sends them, or a
/// pending grant sealed later): the guest's device lists the block while it has no key, then
/// opens it once the key is sealed to it, though it already looked for that key once.
#[tokio::test]
async fn a_key_sealed_after_the_block_was_listed_unlocks_it() {
    let harness = Harness::start().await;
    let owner = harness.owner("owner@example.com").await;
    let guest = harness.member(&owner, "guest@example.com").await;
    // A workspace is a scope root already, so sealing its key is the one row put below.
    let roadmap = workspace(&owner, "Roadmap").await;
    let mut events = guest.connection().subscribe();

    owner
        .grant(roadmap, guest.account(), Access::View)
        .await
        .unwrap();
    let listed = guest.list(Listing::Block(roadmap)).await.unwrap();
    guest.learn(&listed).await.unwrap();
    let [summary] = listed.as_slice() else {
        panic!("the guest lists {listed:?}");
    };
    assert!(
        matches!(guest.metadata(summary), Err(ClientError::Locked(_))),
        "the guest opened the block before its key was sealed to it"
    );

    let public = owner.keys().contact(guest.account()).unwrap();
    owner
        .seal_share(roadmap, guest.account(), &public)
        .await
        .unwrap();
    while !matches!(events.recv().await.unwrap(), Event::KeysChanged) {}
    guest.keys_changed();
    let listed = guest.list(Listing::Block(roadmap)).await.unwrap();
    guest.learn(&listed).await.unwrap();
    assert_eq!(
        guest.metadata(&listed[0]).unwrap().name().as_deref(),
        Some("Roadmap")
    );

    harness.stop().await;
}
