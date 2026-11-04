use be_block_protocol::BrowserLoginState;

use super::*;

/// A browser sign-in is begun and polled with one-shot calls, so while the person approves it the
/// server holds no connection for it, and any number of polls, from anywhere, may come and go.
#[tokio::test]
async fn a_browser_sign_in_waits_on_polls_and_holds_no_connection() {
    let harness = Harness::start().await;
    harness.member("offline@example.com").await;
    let begun = harness
        .once(|peer| client::account::begin_browser_login(peer))
        .await
        .expect("a sign-in begins");
    assert_eq!(
        (begun.interval, begun.expires),
        (
            crate::browser_auth::POLL_INTERVAL.as_secs() as u32,
            crate::browser_auth::LIFETIME.as_secs() as u32
        )
    );
    let (code, secret) = (begun.code, begun.secret);
    assert!(matches!(
        poll_login(&harness, &code, &secret).await,
        Ok(BrowserLoginState::Pending)
    ));
    let mut browser = Browser::default();
    browser
        .login(&harness, "offline@example.com", "correct horse battery")
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
    let soon = poll_login(&harness, &code, &secret).await;
    assert!(
        refused_as(&soon, ErrorCode::SlowDown),
        "a poll sooner than the interval was answered: {soon:?}"
    );

    let mut app = harness.client().await;
    let response = client::account::begin_browser_login(&app.peer).await;
    assert!(
        !matches!(response, Ok(Ok(_))),
        "a connection began a sign-in rather than authenticating: {response:?}"
    );
    app.ended().await;
}
