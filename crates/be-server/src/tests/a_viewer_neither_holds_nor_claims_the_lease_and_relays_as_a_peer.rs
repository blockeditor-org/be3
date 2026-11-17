use super::*;
use be_block_graph::Access;
use be_block_session::Membership;

/// The server cannot read a relayed message, so it stamps the kind it checked on what it
/// delivers. A viewer joining first does not hold the lease, and may neither claim it nor
/// submit; its presence still reaches the others.
#[tokio::test]
async fn a_viewer_neither_holds_nor_claims_the_lease_and_relays_as_a_peer() {
    let harness = Harness::start().await;
    let mut owner = harness.client().await;
    owner.register("owner@example.com").await;
    owner.personal().await;
    let mut guest = harness.client().await;
    let guest_account = guest.register("guest@example.com").await;
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
        .relay(block, Some(owner_id), RelayKind::Peer, b"presence".to_vec())
        .await;
    assert!(matches!(presence, Ok(())), "{presence:?}");
    let heard = loop {
        if let Notice::Relayed {
            from,
            kind,
            payload,
            ..
        } = owner.notification().await
        {
            break (from, kind, payload);
        }
    };
    assert_eq!(heard, (viewer, RelayKind::Peer, b"presence".to_vec()));
    harness.stop().await;
}
