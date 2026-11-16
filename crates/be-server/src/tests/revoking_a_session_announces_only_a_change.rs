use super::*;

#[tokio::test]
async fn revoking_a_session_announces_only_a_change() {
    let harness = Harness::start().await;
    let store = ServerStore::open(&harness.directory).unwrap();
    let (profile, _) = store.register("owner@example.com", "Owner").await.unwrap();
    let browser = store.browser_session(profile.account).await.unwrap();
    // Ending the other sessions needs a fresh proof.
    harness.stale_proofs();
    assert!(
        store
            .revoke_other_sessions(profile.account, &browser)
            .await
            .is_err()
    );
    prove(&store, profile.account, "owner@example.com", &browser).await;
    // The first removes the app session registering made; the second has
    // nothing left to remove.
    store
        .revoke_other_sessions(profile.account, &browser)
        .await
        .unwrap();
    let changes = store.session_changes.subscribe();
    store
        .revoke_session(profile.account, "no-such-session", &browser)
        .await
        .unwrap();
    store
        .revoke_other_sessions(profile.account, &browser)
        .await
        .unwrap();
    assert!(!changes.has_changed().unwrap());
    let sessions = store
        .account_sessions(profile.account, &browser)
        .await
        .unwrap();
    store
        .revoke_session(profile.account, &sessions[0].id, &browser)
        .await
        .unwrap();
    assert!(changes.has_changed().unwrap());
}
