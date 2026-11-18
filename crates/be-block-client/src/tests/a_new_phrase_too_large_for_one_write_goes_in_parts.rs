use super::*;

/// An account signed in more logs than one write of a new phrase carries: its device sends the
/// keys it seals again and the retirements ahead in parts (`stage-recovery-key`), and the last
/// write takes them all as one, so every log is retired and the new phrase is the account's. A
/// write that leaves the parts out is refused whole, as one missing a retirement is.
#[tokio::test]
async fn a_new_phrase_too_large_for_one_write_goes_in_parts() {
    let harness = Harness::start().await;
    let rosa = harness.owner("rosa@example.com").await;
    for name in ["One", "Two", "Three", "Four", "Five"] {
        workspace(&rosa, name).await;
    }
    let read = || async {
        crate::ask(be_block_protocol::read::<be_block_protocol::RecoveryKey, _>(
            &rosa.connection(),
            None,
            be_block_protocol::Query::RecoveryKey,
        ))
        .await
        .unwrap()
    };
    let before = read().await;
    assert!(
        before.logs.len() > 2,
        "rosa signed in several logs: {}",
        before.logs.len()
    );

    // A write that carries none of what the move needs is refused, and changes nothing.
    let stray = be_block_keys::RecoveryPhrase::generate().secret();
    let refused = crate::ask(be_block_protocol::write::<(), _>(
        &rosa.connection(),
        None,
        be_block_protocol::Op::SetRecoveryKey {
            public: stray.public().0,
            signing: stray.signing().0,
            sealed: Vec::new(),
            changes: Vec::new(),
            retirements: Vec::new(),
        },
    ))
    .await;
    assert!(refused.is_err(), "a move retiring no log went through");
    let unsigned = crate::ask(be_block_protocol::write::<(), _>(
        &rosa.connection(),
        None,
        be_block_protocol::Op::StageRecoveryKey(be_block_protocol::RecoveryKeyPart {
            public: stray.public().0,
            signing: stray.signing().0,
            sealed: Vec::new(),
            retirements: vec![be_block_protocol::Retirement {
                organization: rosa.organization(),
                kind: be_block_protocol::LogKind::Block,
                log: Uuid::new_v4(),
                entry: Vec::new(),
            }],
        }),
    ))
    .await;
    assert!(
        unsigned.is_err(),
        "a part staged a retirement of a log rosa never signed in"
    );

    // A budget of one byte puts each key and each retirement in a write of its own.
    let fresh = be_block_keys::RecoveryPhrase::generate().secret();
    crate::contacts::set_recovery_key_in(
        &rosa.connection(),
        rosa.account(),
        Some(rosa.keys().identity()),
        &fresh,
        &std::collections::HashMap::new(),
        1,
    )
    .await
    .unwrap()
    .expect("the old phrase opens everything it sealed");
    let after = read().await;
    assert_eq!(
        after.public,
        Some(fresh.public().0),
        "the new phrase is the account's"
    );
    // Each key sealed to the old phrase is sealed to the new one, beside the account block's
    // new epoch, sealed to the new phrase alone.
    assert_eq!(
        after.sealed.len(),
        before.sealed.len() + 1,
        "a key sealed to the old phrase was not sealed to the new one"
    );

    harness.stop().await;
}
