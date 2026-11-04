use super::*;

#[tokio::test]
async fn password_reset_is_generic_and_revokes_live_sessions_and_pending_approvals() {
    let harness = Harness::start().await;
    let mut app = harness.member("reset@example.com").await;
    let (code, secret) = begin_session(&mut app).await;
    let mut browser = Browser::default();
    browser
        .login(&harness, "reset@example.com", "correct horse battery")
        .await;
    assert_eq!(
        browser
            .submit(
                &harness,
                &format!("/auth?code={code}"),
                "/auth/approve",
                &[("code", &code)]
            )
            .await
            .0,
        303
    );
    for email in ["reset@example.com", "missing@example.com"] {
        let (status, response) = browser
            .submit(
                &harness,
                "/auth/forgot",
                "/auth/forgot",
                &[("email", email)],
            )
            .await;
        assert_eq!(status, 303);
        assert!(response.contains("Location: /auth/reset-requested"));
    }
    let token = harness.mail_token("reset@example.com");
    let previous = browser.cookie.clone();
    assert_eq!(
        browser
            .submit(
                &harness,
                "/auth/reset",
                "/auth/reset",
                &[
                    ("token", &token),
                    ("password", "a new password"),
                    ("confirmation", "a new password")
                ]
            )
            .await
            .0,
        303
    );
    let store = ServerStore::open(&harness.directory).unwrap();
    assert!(store.browser_profile(&previous).await.is_err());
    assert!(store.resolve_token(&app.token).await.is_err());
    assert!(
        store
            .login_browser("reset@example.com", "correct horse battery")
            .await
            .is_err()
    );
    assert!(
        store
            .login_browser("reset@example.com", "a new password")
            .await
            .is_ok()
    );
    app.ended().await;
    let mut claimant = harness.client().await;
    assert!(matches!(
        claimant
            .call(|peer| client::account::finish_browser_login(peer, code, secret))
            .await,
        Err(_)
    ));
    assert_eq!(
        browser
            .submit(
                &harness,
                "/auth/reset",
                "/auth/reset",
                &[
                    ("token", &token),
                    ("password", "a third password"),
                    ("confirmation", "a third password")
                ]
            )
            .await
            .0,
        400
    );
}
