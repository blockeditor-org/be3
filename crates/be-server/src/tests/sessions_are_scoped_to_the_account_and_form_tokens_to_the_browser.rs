use super::*;

#[tokio::test]
async fn sessions_are_scoped_to_the_account_and_form_tokens_to_the_browser() {
    let harness = Harness::start().await;
    let owner = harness.member("owner@example.com").await;
    harness.member("stranger@example.com").await;
    let mut browser = Browser::default();
    let mut stranger = Browser::default();
    browser.sign_in_by_code(&harness, "owner@example.com").await;
    stranger
        .sign_in_by_code(&harness, "stranger@example.com")
        .await;
    let store = ServerStore::open(&harness.directory).unwrap();
    let profile = store.browser_profile(&browser.cookie).await.unwrap();
    let sessions = store
        .account_sessions(profile.account, &browser.cookie)
        .await
        .unwrap();
    let app = sessions
        .iter()
        .find(|session| session.kind == "app")
        .unwrap();
    // Even proved, the stranger's revoking reaches only its own sessions.
    stranger.prove(&harness, "stranger@example.com").await;
    assert_eq!(
        stranger
            .submit(
                &harness,
                "/auth/sessions",
                "/auth/sessions/revoke",
                &[("session", &app.id)]
            )
            .await
            .0,
        303
    );
    assert!(store.resolve_token(&owner.token).await.is_ok());
    let page = browser.get(&harness, "/auth/settings").await;
    assert_eq!(
        stranger
            .request(
                &harness,
                "POST",
                "/auth/settings/name",
                &format!("csrf={}&display_name=Stolen", csrf(&page))
            )
            .await
            .0,
        400
    );
    browser.prove(&harness, "owner@example.com").await;
    assert_eq!(
        browser
            .submit(
                &harness,
                "/auth/sessions",
                "/auth/sessions/revoke",
                &[("session", &app.id)]
            )
            .await
            .0,
        303
    );
    assert!(store.resolve_token(&owner.token).await.is_err());
}
