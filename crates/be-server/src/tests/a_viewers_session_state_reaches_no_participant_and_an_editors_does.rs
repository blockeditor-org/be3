use super::*;
use be_block_graph::Access;

/// The server cannot read a relayed message, so it checks the kind its sender names: what
/// carries operations or session state only from an editor, and stamps what it checked on what
/// it delivers. A viewer joining first is not granted the lease.
#[tokio::test]
async fn a_viewers_session_state_reaches_no_participant_and_an_editors_does() {
    let harness = Harness::start().await;
    let mut owner = harness.client().await;
    owner.register("owner@example.com").await;
    let organization = owner.personal().await;
    let mut guest = harness.client().await;
    let guest_account = guest.register("guest@example.com").await;
    guest.personal().await;
    let link = owner.link(None).await;
    let _ = guest.accept(link, &[]).await;
    let block = owner.create_block(BlockParent::Root).await;
    let granted = owner.grant(block, guest_account, Access::View).await;
    assert!(matches!(granted, Ok(())), "{granted:?}");

    let (viewer, state) = guest.join_session(block).await;
    assert_eq!(state.owner, None, "a viewer was granted the lease");
    let (owner_id, state) = owner.join_session(block).await;
    assert_eq!(state.owner, Some(owner_id));
    let mut editor = harness
        .second_connection(&owner.token.clone(), organization)
        .await;
    let (editor_id, _) = editor.join_session(block).await;

    let forged = guest
        .relay(block, None, RelayKind::Edit, b"forged".to_vec())
        .await;
    assert!(
        refused_as(&forged, ErrorCode::PermissionDenied),
        "a viewer relayed an edit: {forged:?}"
    );

    let presence = guest
        .relay(block, Some(owner_id), RelayKind::Peer, b"presence".to_vec())
        .await;
    assert!(matches!(presence, Ok(())), "{presence:?}");
    let submit = editor
        .relay(block, Some(owner_id), RelayKind::Edit, b"submit".to_vec())
        .await;
    assert!(matches!(submit, Ok(())), "{submit:?}");
    let accepted = owner
        .relay(
            block,
            Some(editor_id),
            RelayKind::Edit,
            b"accepted".to_vec(),
        )
        .await;
    assert!(matches!(accepted, Ok(())), "{accepted:?}");

    assert_eq!(
        relayed(&mut owner).await,
        (viewer, RelayKind::Peer, b"presence".to_vec())
    );
    assert_eq!(
        relayed(&mut owner).await,
        (editor_id, RelayKind::Edit, b"submit".to_vec())
    );
    assert_eq!(
        relayed(&mut editor).await,
        (owner_id, RelayKind::Edit, b"accepted".to_vec())
    );
    harness.stop().await;
}

/// Who relayed the next message, the kind the server stamped it with, and its payload.
async fn relayed(client: &mut TestClient) -> (ClientId, RelayKind, Vec<u8>) {
    loop {
        if let Notice::Relayed {
            from,
            kind,
            payload,
            ..
        } = client.notification().await
        {
            return (from, kind, payload);
        }
    }
}
