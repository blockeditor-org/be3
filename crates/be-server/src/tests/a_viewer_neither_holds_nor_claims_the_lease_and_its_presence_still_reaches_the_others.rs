use super::*;
use be_block_graph::Access;
use be_block_session::Membership;

/// A viewer joining first does not hold the lease, and may neither claim it nor submit; its
/// presence still reaches the others.
#[tokio::test]
async fn a_viewer_neither_holds_nor_claims_the_lease_and_its_presence_still_reaches_the_others() {
    let harness = Harness::start().await;
    let mut owner = harness.member("owner@example.com").await;
    owner.personal().await;
    let mut guest = harness.member("guest@example.com").await;
    let guest_account = guest.id();
    guest.personal().await;
    let link = owner.link(None).await;
    let _ = guest.accept(link, &[]).await;
    let block = owner.create_block(BlockParent::Root).await;
    let granted = owner.grant(block, guest_account, Access::View).await;
    assert!(matches!(granted, Ok(())), "{granted:?}");

    let (viewer, state) = guest.join_session(block).await;
    assert_eq!(state.holder(), None, "a viewer holds the lease");
    let (owner_id, state) = owner.join_session(block).await;
    assert!(state.is_holder(owner_id));

    let claimed = guest.claim(block).await;
    assert!(
        refused_as(&claimed, ErrorCode::PermissionDenied),
        "a viewer claimed the lease: {claimed:?}"
    );
    let submitted = guest.submit(block, 1, 1, 0, b"an edit".to_vec()).await;
    assert!(
        refused_as(&submitted, ErrorCode::PermissionDenied),
        "a viewer submitted: {submitted:?}"
    );

    let presence = guest
        .relay(block, Some(owner_id), b"presence".to_vec())
        .await;
    assert!(matches!(presence, Ok(())), "{presence:?}");
    let heard = loop {
        if let Notice::Relayed { from, payload, .. } = owner.notification().await {
            break (from, payload);
        }
    };
    assert_eq!(heard, (viewer, b"presence".to_vec()));
    harness.stop().await;
}
