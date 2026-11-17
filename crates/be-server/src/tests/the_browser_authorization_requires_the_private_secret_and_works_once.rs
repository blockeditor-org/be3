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
            ],
        )
        .await;
    assert_eq!(status, 303, "{page}");
    assert!(
        matches!(
            poll_login(&harness, &code, &secret).await,
            Ok(BrowserLoginState::Pending)
        ),
        "a sign-in finished before the browser approved it"
    );
    let (email_code, _) = harness.mailed_code("ada@example.com");
    let (status, page) = browser.enter_code(&harness, &email_code).await;
    assert_eq!(status, 303, "{page}");
    // Entering the code signs the browser in and returns it to the sign-in, which still waits
    // for the person to approve it.
    assert!(page.contains(&format!("/auth?code={code}")), "{page}");
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
    assert!(harness.sign_in(&token).await.is_ok());
}
