use super::*;

#[tokio::test]
async fn password_changes_rotate_the_browser_and_revoke_other_sessions() {
    let harness = Harness::start().await;
    let app = harness.member("change@example.com").await;
    let mut browser = Browser::default();
    browser
        .login(&harness, "change@example.com", "correct horse battery")
        .await;
    let previous = browser.cookie.clone();
    assert_eq!(
        browser
            .submit(
                &harness,
                "/auth/settings",
                "/auth/settings/password",
                &[
                    ("current_password", "correct horse battery"),
                    ("password", "a new password"),
                    ("confirmation", "a new password")
                ]
            )
            .await
            .0,
        303
    );
    assert_ne!(browser.cookie, previous);
    let store = ServerStore::open(&harness.directory).unwrap();
    assert!(store.browser_profile(&previous).await.is_err());
    assert!(store.resolve_token(&app.token).await.is_err());
    assert!(store.browser_profile(&browser.cookie).await.is_ok());
}
