use super::*;

use be_block_graph::Access;

/// A block offered to a member who then loses access can no longer be taken by them, whether
/// they are revoked on the block itself, which its log shows, or on a block above it, which
/// only the server knows: the offer is no longer listed to them and their acceptance is refused
/// (guides/blocks/keys.md, "Owner records").
#[tokio::test]
async fn an_offer_of_ownership_ends_with_the_access_of_whom_it_was_made_to() {
    let harness = Harness::start().await;
    let admin = harness.owner("admin@example.com").await;
    let alice = harness.member(&admin, "alice@example.com").await;
    let bob = harness.member(&admin, "bob@example.com").await;
    befriend(&alice, &bob).await;
    let plans = workspace(&alice, "Plans").await;
    let note = alice
        .create::<TextContent>(BlockParent::Block(plans))
        .await
        .unwrap();
    let bobs = alice.verified_identity(bob.account()).unwrap();

    // Revoked on the block offered.
    alice
        .share(plans, bob.account(), Access::Edit, &bobs)
        .await
        .unwrap();
    alice.transfer(plans, bob.account()).await.unwrap();
    assert_eq!(bob.owner_offers().await.unwrap().len(), 1);
    alice
        .share(plans, bob.account(), Access::None, &bobs)
        .await
        .unwrap();
    assert!(
        bob.owner_offers().await.unwrap().is_empty(),
        "an offer to a revoked member is listed"
    );
    assert!(
        bob.accept_ownership(plans).await.is_err(),
        "a revoked member takes the block"
    );

    // Revoked on the block above the one offered.
    alice
        .share(plans, bob.account(), Access::Edit, &bobs)
        .await
        .unwrap();
    alice.transfer(note, bob.account()).await.unwrap();
    assert_eq!(bob.owner_offers().await.unwrap().len(), 1);
    alice
        .share(plans, bob.account(), Access::None, &bobs)
        .await
        .unwrap();
    assert!(
        bob.owner_offers().await.unwrap().is_empty(),
        "an offer to a member who can no longer read the block is listed"
    );
    assert!(
        bob.accept_ownership(note).await.is_err(),
        "a member who can no longer read the block takes it"
    );
    let (_, checked) = alice.checked_access(note).await.unwrap();
    assert_eq!(checked.owner, Some(alice.account().into_bytes()));

    harness.stop().await;
}
