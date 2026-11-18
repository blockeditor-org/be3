use super::*;

#[tokio::test]
async fn a_passkey_signs_in_with_no_email_typed() {
    let harness = Harness::start().await;
    harness.member("ada@example.com").await;
    let mut ada = Browser::at_localhost(&harness);
    ada.sign_in_by_code(&harness, "ada@example.com").await;
    // Signing in just now is a fresh proof, so the settings page offers to add one.
    let mut authenticator = ada.add_passkey(&harness).await;
    assert_eq!(
        harness.mailed(
            "ada@example.com",
            "A way to sign in to your be3 account was added"
        ),
        1
    );
    let page = page_of(&ada.get(&harness, "/auth/settings").await);
    let passkey = page
        .identities
        .iter()
        .find(|identity| identity.kind == "passkey")
        .expect("the passkey is listed");
    assert!(passkey.label.starts_with("Passkey added "), "{passkey:?}");
    assert!(passkey.removable);
    // Another browser signs in with it from the sign-in page, before any email is typed, and
    // that sign-in is a fresh proof too.
    let mailed = harness.mails_to("ada@example.com");
    let mut elsewhere = Browser::at_localhost(&harness);
    let (status, response) = elsewhere
        .sign_in_with_passkey(&harness, &mut authenticator)
        .await;
    assert_eq!(status, 303, "{response}");
    assert_eq!(location(&response), "/auth?code=");
    let page = page_of(&elsewhere.get(&harness, "/auth").await);
    assert_eq!(page.screen, be_server_portal::Screen::Account);
    assert_eq!(page.account.unwrap().email, "ada@example.com");
    assert!(page_of(&elsewhere.get(&harness, "/auth/settings").await).fresh);
    assert_eq!(harness.mails_to("ada@example.com"), mailed);
    // It also approves a be-host's sign-in, carrying its code through.
    let (code, secret) = begin_session(&harness).await;
    let mut device = Browser::at_localhost(&harness);
    let origin = device.origin();
    let (status, response) = device
        .passkey(&harness, &format!("/auth?code={code}"), |options| {
            authenticator.get(options, &origin)
        })
        .await;
    assert_eq!(status, 303, "{response}");
    assert_eq!(location(&response), format!("/auth?code={code}"));
    let (status, response) = device
        .submit(
            &harness,
            &location(&response),
            "/auth/approve",
            &[("code", &code)],
        )
        .await;
    assert_eq!(status, 303, "{response}");
    assert!(matches!(
        poll_login(&harness, &code, &secret).await,
        Ok(be_block_protocol::BrowserLoginState::Approved(Authenticated { email, .. }))
            if email == "ada@example.com"
    ));
}
