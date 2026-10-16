use super::*;

use be_protocol::SealedKey;

#[tokio::test]
async fn workspace_keys_are_sealed_per_member_and_never_overwritten() {
    let harness = Harness::start().await;
    let mut owner = harness.client().await;
    owner.register("owner@example.com").await;
    let workspace = owner.workspace("notes").await;

    let refused = owner
        .send(|request| ClientMessage::PutWorkspaceKey {
            request,
            workspace,
            sealed: b"sealed for the owner".to_vec(),
        })
        .await;
    assert!(
        matches!(refused, ServerMessage::Failed { .. }),
        "a key was sealed for an account with no recovery key: {refused:?}"
    );

    let response = owner
        .send(|request| ClientMessage::SetRecoveryKey {
            request,
            public: [1; 32],
            sealed: Vec::new(),
        })
        .await;
    assert!(matches!(response, ServerMessage::Ok { .. }), "{response:?}");
    for sealed in [b"sealed for the owner".to_vec(), b"a forgery".to_vec()] {
        let response = owner
            .send(|request| ClientMessage::PutWorkspaceKey {
                request,
                workspace,
                sealed: sealed.clone(),
            })
            .await;
        assert!(matches!(response, ServerMessage::Ok { .. }), "{response:?}");
    }
    let keys = owner
        .send(|request| ClientMessage::GetKeys { request })
        .await;
    let ServerMessage::Keys {
        recovery, sealed, ..
    } = keys
    else {
        panic!("reading keys failed: {keys:?}");
    };
    assert_eq!(recovery, Some([1; 32]));
    assert_eq!(
        sealed,
        vec![SealedKey {
            workspace,
            sealed: b"sealed for the owner".to_vec(),
        }],
        "a second put overwrote the sealed key"
    );

    let mut guest = harness.member("guest@example.com").await;
    guest
        .send(|request| ClientMessage::SetRecoveryKey {
            request,
            public: [2; 32],
            sealed: Vec::new(),
        })
        .await;
    let refused = guest
        .send(|request| ClientMessage::PutWorkspaceKey {
            request,
            workspace,
            sealed: b"sealed by an outsider".to_vec(),
        })
        .await;
    assert!(
        matches!(refused, ServerMessage::Failed { .. }),
        "an outsider stored a key for a workspace: {refused:?}"
    );

    owner
        .send(|request| ClientMessage::Invite {
            request,
            workspace,
            email: "guest@example.com".into(),
            role: WorkspaceRole::Editor,
            sealed: Vec::new(),
        })
        .await;
    let ServerMessage::Invitations { invitations, .. } = guest
        .send(|request| ClientMessage::ListInvitations { request })
        .await
    else {
        panic!("listing invitations failed");
    };
    guest
        .send(|request| ClientMessage::RespondInvitation {
            request,
            invitation: invitations[0].id,
            accept: true,
        })
        .await;
    let response = guest
        .send(|request| ClientMessage::PutWorkspaceKey {
            request,
            workspace,
            sealed: b"sealed for the guest".to_vec(),
        })
        .await;
    assert!(matches!(response, ServerMessage::Ok { .. }), "{response:?}");
    let keys = guest
        .send(|request| ClientMessage::GetKeys { request })
        .await;
    assert!(
        matches!(&keys, ServerMessage::Keys { sealed, .. }
            if sealed == &vec![SealedKey { workspace, sealed: b"sealed for the guest".to_vec() }]),
        "{keys:?}"
    );

    let refused = owner
        .send(|request| ClientMessage::SetRecoveryKey {
            request,
            public: [3; 32],
            sealed: Vec::new(),
        })
        .await;
    assert!(
        matches!(refused, ServerMessage::Failed { .. }),
        "a new recovery key dropped a workspace key: {refused:?}"
    );
    let response = owner
        .send(|request| ClientMessage::SetRecoveryKey {
            request,
            public: [3; 32],
            sealed: vec![SealedKey {
                workspace,
                sealed: b"resealed for the owner".to_vec(),
            }],
        })
        .await;
    assert!(matches!(response, ServerMessage::Ok { .. }), "{response:?}");
    let keys = owner
        .send(|request| ClientMessage::GetKeys { request })
        .await;
    assert!(
        matches!(&keys, ServerMessage::Keys { recovery: Some([3, ..]), sealed, .. }
            if sealed[0].sealed == b"resealed for the owner"),
        "{keys:?}"
    );

    harness.stop().await;
}
