use super::*;

/// A move staged in parts, then a key sealed to the account before the last write: that key
/// is in no part, so the last write is refused and the old phrase stays the account's.
#[tokio::test]
async fn a_key_sealed_between_staging_and_the_last_write_refuses_the_move() {
    let harness = Harness::start().await;
    let rosa = harness.owner("rosa@example.com").await;
    workspace(&rosa, "One").await;
    let read = || async {
        crate::ask(
            be_block_protocol::read::<be_block_protocol::RecoveryKey, _>(
                &rosa.connection(),
                None,
                be_block_protocol::Query::RecoveryKey,
            ),
        )
        .await
        .unwrap()
    };
    let before = read().await;
    let fresh = be_block_keys::RecoveryPhrase::generate().secret();
    let resealed = crate::contacts::new_identity(
        rosa.account(),
        rosa.keys().identity(),
        &fresh,
        before.sealed.clone(),
        &std::collections::HashMap::new(),
        before.logs.clone(),
    )
    .expect("the old phrase opens everything it sealed");
    crate::ask(be_block_protocol::write::<(), _>(
        &rosa.connection(),
        None,
        be_block_protocol::Op::StageRecoveryKey(be_block_protocol::RecoveryKeyPart {
            public: fresh.public().0,
            signing: fresh.signing().0,
            sealed: resealed.sealed,
            retirements: resealed.retirements,
        }),
    ))
    .await
    .unwrap();

    workspace(&rosa, "Two").await;
    let last = crate::ask(be_block_protocol::write::<(), _>(
        &rosa.connection(),
        None,
        be_block_protocol::Op::SetRecoveryKey {
            public: fresh.public().0,
            signing: fresh.signing().0,
            sealed: Vec::new(),
            changes: Vec::new(),
            retirements: Vec::new(),
        },
    ))
    .await;
    assert!(
        last.is_err(),
        "a move went through without the key sealed after its parts"
    );
    assert_eq!(
        read().await.public,
        before.public,
        "the refused move changed the account's phrase"
    );

    harness.stop().await;
}
