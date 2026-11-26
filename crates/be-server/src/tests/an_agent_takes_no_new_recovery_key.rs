use super::*;

/// An agent's keys are its maker's, which makes the agent again rather than move it, so the
/// server refuses an agent a new recovery key, staged or whole: no contact then ever seals to
/// a key an agent replaced (guides/blocks/keys.md, "Contacts").
#[tokio::test]
async fn an_agent_takes_no_new_recovery_key() {
    let harness = Harness::start().await;
    let mut creator = harness.member("creator@example.com").await;
    let personal = creator.personal().await;
    creator.open(personal).await;
    let response = creator.agent(personal, "Helper").await;
    let Ok(be_block_protocol::CreatedAgent { token, .. }) = response else {
        panic!("creating an agent failed: {response:?}");
    };
    let mut agent = harness.second_connection(&token, personal).await;
    let fresh = be_block_keys::RecoveryPhrase::generate()
        .secret()
        .identity();

    let staged = agent
        .call(|peer| {
            peer.write::<()>(Op::StageRecoveryKey(be_block_protocol::RecoveryKeyPart {
                public: fresh.public.0,
                signing: fresh.signing.0,
                sealed: Vec::new(),
                retirements: Vec::new(),
            }))
        })
        .await;
    assert!(
        matches!(&staged, Err(error) if error.code == ErrorCode::PermissionDenied),
        "an agent staged a new recovery key: {staged:?}"
    );
    let moved = agent
        .call(|peer| {
            peer.write::<()>(Op::SetRecoveryKey {
                public: fresh.public.0,
                signing: fresh.signing.0,
                sealed: Vec::new(),
                changes: Vec::new(),
                retirements: Vec::new(),
            })
        })
        .await;
    assert!(
        matches!(&moved, Err(error) if error.code == ErrorCode::PermissionDenied),
        "an agent took a new recovery key: {moved:?}"
    );

    harness.stop().await;
}
