use super::*;
use be_protocol::SealedKey;

async fn recovery(client: &mut TestClient) -> (Option<[u8; 32]>, Vec<SealedKey>) {
    let response = client
        .send(|request| ClientMessage::GetRecoveryKey { request })
        .await;
    let ServerMessage::RecoveryKey { public, sealed, .. } = response else {
        panic!("reading the recovery key failed: {response:?}");
    };
    (public, sealed)
}

#[tokio::test]
async fn a_new_recovery_key_reseals_every_key_sealed_to_the_account() {
    let harness = Harness::start().await;
    let mut owner = harness.client().await;
    let account = owner.register("owner@example.com").await;
    let organization = owner.organization("notes").await;
    assert_eq!(recovery(&mut owner).await, (None, Vec::new()));

    let response = owner
        .send(|request| ClientMessage::SetRecoveryKey {
            request,
            public: [1; 32],
            sealed: Vec::new(),
            own: Vec::new(),
            changes: Vec::new(),
        })
        .await;
    assert!(matches!(response, ServerMessage::Ok { .. }), "{response:?}");
    let workspace = owner.scope_root(account, BlockParent::Root).await;
    let (public, sealed) = recovery(&mut owner).await;
    assert_eq!(public, Some([1; 32]));
    assert_eq!(
        sealed,
        vec![SealedKey {
            organization,
            key: workspace.scope,
            sender: account,
            sealed: b"scope sealed to its author".to_vec(),
        }]
    );

    let response = owner
        .send(|request| ClientMessage::PutKeys {
            request,
            keys: KeySet {
                keys: Vec::new(),
                wrapped: vec![wrap(
                    workspace.scope,
                    WrappedBy::Account(account),
                    b"a forgery",
                )],
            },
        })
        .await;
    assert!(matches!(response, ServerMessage::Ok { .. }), "{response:?}");
    assert_eq!(
        recovery(&mut owner).await.1[0].sealed,
        b"scope sealed to its author",
        "a second put overwrote a sealed key"
    );

    let refused = owner
        .send(|request| ClientMessage::SetRecoveryKey {
            request,
            public: [2; 32],
            sealed: Vec::new(),
            own: Vec::new(),
            changes: Vec::new(),
        })
        .await;
    assert!(
        matches!(refused, ServerMessage::Failed { .. }),
        "a new recovery key left a key sealed to the old one: {refused:?}"
    );
    assert_eq!(recovery(&mut owner).await.0, Some([1; 32]));

    let response = owner
        .send(|request| ClientMessage::SetRecoveryKey {
            request,
            public: [2; 32],
            sealed: vec![SealedKey {
                organization,
                key: workspace.scope,
                sender: account,
                sealed: b"resealed to the new phrase".to_vec(),
            }],
            own: Vec::new(),
            changes: Vec::new(),
        })
        .await;
    assert!(matches!(response, ServerMessage::Ok { .. }), "{response:?}");
    assert_eq!(
        recovery(&mut owner).await,
        (
            Some([2; 32]),
            vec![SealedKey {
                organization,
                key: workspace.scope,
                sender: account,
                sealed: b"resealed to the new phrase".to_vec(),
            }]
        )
    );

    harness.stop().await;
}
