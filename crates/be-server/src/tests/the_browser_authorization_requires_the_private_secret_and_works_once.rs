use be_block_protocol::BrowserLoginState;

use super::*;

#[tokio::test]
async fn the_browser_authorization_requires_the_private_secret_and_works_once() {
    let harness = Harness::configured(ServerConfig {
        sign_in_poll_interval: Duration::ZERO,
        ..ServerConfig::OPEN
    })
    .await;
    let (code, secret) = begin_session(&harness).await;
    let mut browser = Browser::default();
    let (status, page) = browser
        .submit(
            &harness,
            "/auth/register",
            "/auth/register",
            &[
                ("code", &code),
                ("invite", &harness.invite()),
                ("email", "ada@example.com"),
                ("display_name", "Ada"),
                ("password", "correct horse battery"),
                ("confirmation", "correct horse battery"),
            ],
        )
        .await;
    assert_eq!(status, 303, "{page}");
    assert!(!page.contains("correct horse"));
    assert!(
        matches!(
            poll_login(&harness, &code, &secret).await,
            Ok(BrowserLoginState::Pending)
        ),
        "a sign-in finished before the browser approved it"
    );
    let token = harness.mail_token("ada@example.com");
    let path = format!("/auth/verify?token={token}");
    assert_eq!(
        browser
            .submit(&harness, &path, "/auth/verify", &[("token", &token)])
            .await
            .0,
        303
    );
    assert!(
        matches!(
            poll_login(&harness, &code, &secret).await,
            Ok(BrowserLoginState::Pending)
        ),
        "a sign-in finished before the browser approved it"
    );
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
    assert!(
        matches!(
            poll_login(&harness, &code, "wrong secret").await,
            Ok(BrowserLoginState::Expired)
        ),
        "a wrong secret read the sign-in"
    );
    let approved = poll_login(&harness, &code, &secret).await;
    let Ok(BrowserLoginState::Approved(Authenticated { token, email, .. })) = approved else {
        panic!("{approved:?}")
    };
    assert_eq!(email, "ada@example.com");
    assert!(
        matches!(
            poll_login(&harness, &code, &secret).await,
            Ok(BrowserLoginState::Expired)
        ),
        "the token was handed out twice"
    );
    let mut other = harness.client().await;
    assert!(matches!(
        other
            .call(|peer| client::account::authenticate(peer, Some(token)))
            .await,
        Ok(_)
    ));
}
