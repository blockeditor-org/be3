use super::*;

#[tokio::test]
async fn a_remembered_account_approves_a_manually_entered_code_and_can_switch_accounts() {
    let harness = Harness::start().await;
    harness.member("first@example.com").await;
    harness.member("second@example.com").await;
    let mut app = harness.client().await;
    let (code, secret) = begin_session(&mut app).await;
    let mut browser = Browser::default();
    browser
        .login(&harness, "first@example.com", "correct horse battery")
        .await;
    let (status, _) = browser
        .submit(
            &harness,
            "/auth",
            "/auth/code",
            &[("code", &code.to_ascii_uppercase())],
        )
        .await;
    assert_eq!(status, 303);
    let page = browser.get(&harness, &format!("/auth?code={code}")).await;
    let page = page_of(&page);
    assert_eq!(page.account.unwrap().email, "first@example.com");
    assert_eq!(page.screen, be_portal::Screen::Approve);
    let previous = browser.cookie.clone();
    assert_eq!(
        browser
            .submit(
                &harness,
                &format!("/auth?code={code}"),
                "/auth/logout",
                &[("code", &code)]
            )
            .await
            .0,
        303
    );
    assert!(browser.cookie.is_empty());
    assert!(
        ServerStore::open(&harness.directory)
            .unwrap()
            .browser_profile(&previous)
            .await
            .is_err()
    );
    browser
        .login(&harness, "second@example.com", "correct horse battery")
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
    assert!(
        matches!(app.call(|peer| client::account::finish_browser_login(peer, code, secret)).await, Ok(Authenticated { email, .. }) if email == "second@example.com")
    );
}
