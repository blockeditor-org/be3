use super::*;

/// Whoever holds a signed-in browser's cookie, or phishes its owner to the approve page long
/// after they signed in, gets no app token for the account without proving it is them again
/// (guides/blocks/server.md, "Accounts").
#[tokio::test]
async fn approving_a_device_code_needs_a_fresh_proof() {
    let harness = Harness::configured(ServerConfig {
        sign_in_poll_interval: Duration::ZERO,
        ..ServerConfig::OPEN
    })
    .await;
    harness.member("owner@example.com").await;
    let mut browser = Browser::default();
    browser.sign_in_by_code(&harness, "owner@example.com").await;
    harness.stale_proofs();
    let (code, secret) = begin_session(&harness).await;
    let page = page_of(&browser.get(&harness, &format!("/auth?code={code}")).await);
    assert_eq!(page.screen, be_server_portal::Screen::Approve);
    // It names where the device asking connected from, as the server saw it.
    assert!(
        page.requester
            .ends_with("the same network as this browser.")
            && !page.requester_elsewhere,
        "{:?}",
        page.requester
    );
    browser
        .submit(
            &harness,
            &format!("/auth?code={code}"),
            "/auth/approve",
            &[("code", &code)],
        )
        .await;
    let claimed = poll_login(&harness, &code, &secret).await;
    assert!(
        !matches!(
            claimed,
            Ok(be_block_protocol::BrowserLoginState::Approved(_))
        ),
        "a browser with a stale proof approved a device: {claimed:?}"
    );
    let page = page_of(&browser.get(&harness, &format!("/auth?code={code}")).await);
    assert!(!page.fresh, "the page asks to sign in again");
    // Signing in again keeps the code, and approves.
    let (status, _) = browser
        .submit(
            &harness,
            &format!("/auth?code={code}"),
            "/auth/logout",
            &[("code", &code)],
        )
        .await;
    assert_eq!(status, 303);
    browser.sign_in_by_code(&harness, "owner@example.com").await;
    let page = page_of(&browser.get(&harness, &format!("/auth?code={code}")).await);
    assert_eq!(page.screen, be_server_portal::Screen::Approve);
    assert!(page.fresh);
    let (status, _) = browser
        .submit(
            &harness,
            &format!("/auth?code={code}"),
            "/auth/approve",
            &[("code", &code)],
        )
        .await;
    assert_eq!(status, 303);
    assert!(matches!(
        poll_login(&harness, &code, &secret).await,
        Ok(be_block_protocol::BrowserLoginState::Approved(_))
    ));
}
