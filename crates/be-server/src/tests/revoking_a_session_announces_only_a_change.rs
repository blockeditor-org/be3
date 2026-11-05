use super::*;

#[tokio::test]
async fn revoking_a_session_announces_only_a_change() {
    let harness = Harness::start().await;
    let store = ServerStore::open(&harness.directory).unwrap();
    let (profile, _) = store
        .register("owner@example.com", "Owner", "a good password")
        .await
        .unwrap();
    let browser = store.browser_session(profile.account).await.unwrap();
    // The first removes the app session registering made; the second has
    // nothing left to remove.
    store
        .revoke_other_sessions(profile.account, &browser)
        .await
        .unwrap();
    let changes = store.session_changes.subscribe();
    store
        .revoke_session(profile.account, "no-such-session")
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
        .revoke_session(profile.account, &sessions[0].id)
        .await
        .unwrap();
    assert!(changes.has_changed().unwrap());
}
