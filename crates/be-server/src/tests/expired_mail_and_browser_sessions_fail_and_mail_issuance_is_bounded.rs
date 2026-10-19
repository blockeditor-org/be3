use super::*;

#[tokio::test]
async fn expired_mail_and_browser_sessions_fail_and_mail_issuance_is_bounded() {
    let harness = Harness::start().await;
    let store = ServerStore::open(&harness.directory).unwrap();
    let (profile, _) = store
        .register("expire@example.com", "Expiry", "a good password")
        .await
        .unwrap();
    let browser = store.browser_session(profile.account).await.unwrap();
    let link = store
        .password_reset_link("expire@example.com", "")
        .await
        .unwrap()
        .unwrap();
    assert!(
        store
            .password_reset_link("expire@example.com", "")
            .await
            .unwrap()
            .is_some()
    );
    assert!(
        store
            .password_reset_link("expire@example.com", "")
            .await
            .unwrap()
            .is_some()
    );
    assert!(
        store
            .password_reset_link("expire@example.com", "")
            .await
            .unwrap()
            .is_none()
    );
    let database = rusqlite::Connection::open(harness.directory.join("metadata.sqlite")).unwrap();
    database
        .execute("UPDATE account_mail SET expires = 0", [])
        .unwrap();
    database
        .execute("UPDATE sessions SET expires = 0 WHERE kind = 'browser'", [])
        .unwrap();
    assert!(store.browser_profile(&browser).await.is_err());
    assert!(
        store
            .confirm_mail(&link.token, Some("another password"))
            .await
            .is_err()
    );
}
