use super::*;

use be_protocol::SealedKey;

#[tokio::test]
async fn organization_keys_are_sealed_per_member_and_never_overwritten() {
    let harness = Harness::start().await;
    let mut owner = harness.client().await;
    owner.register("owner@example.com").await;
    let organization = owner.organization("notes").await;

    let refused = owner
        .send(|request| ClientMessage::PutOrganizationKey {
            request,
            organization,
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
            .send(|request| ClientMessage::PutOrganizationKey {
                request,
                organization,
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
            organization,
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
        .send(|request| ClientMessage::PutOrganizationKey {
            request,
            organization,
            sealed: b"sealed by an outsider".to_vec(),
        })
        .await;
    assert!(
        matches!(refused, ServerMessage::Failed { .. }),
        "an outsider stored a key for an organization: {refused:?}"
    );

    owner
        .send(|request| ClientMessage::Invite {
            request,
            invitation: Uuid::from_u128(1),
            organization,
            email: "guest@example.com".into(),
            role: OrganizationRole::Editor,
            sealed: Vec::new(),
        })
        .await;
    guest
        .send(|request| ClientMessage::AcceptInvitation {
            request,
            invitation: Uuid::from_u128(1),
        })
        .await;
    let response = guest
        .send(|request| ClientMessage::PutOrganizationKey {
            request,
            organization,
            sealed: b"sealed for the guest".to_vec(),
        })
        .await;
    assert!(matches!(response, ServerMessage::Ok { .. }), "{response:?}");
    let keys = guest
        .send(|request| ClientMessage::GetKeys { request })
        .await;
    assert!(
        matches!(&keys, ServerMessage::Keys { sealed, .. }
            if sealed == &vec![SealedKey { organization, sealed: b"sealed for the guest".to_vec() }]),
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
        "a new recovery key dropped an organization key: {refused:?}"
    );
    let response = owner
        .send(|request| ClientMessage::SetRecoveryKey {
            request,
            public: [3; 32],
            sealed: vec![SealedKey {
                organization,
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
