use super::*;
use crate::fake_authenticator::FakeAuthenticator;

#[tokio::test]
async fn a_passkey_for_another_rp_id_or_origin_is_refused() {
    let harness = Harness::start().await;
    harness.member("ada@example.com").await;
    // At an IP address there is no RP ID, so no page offers a passkey.
    let mut plain = Browser::default();
    assert_eq!(
        page_of(&plain.get(&harness, "/auth/login").await).passkey,
        None
    );
    let mut ada = Browser::at_localhost(&harness);
    ada.sign_in_by_code(&harness, "ada@example.com").await;
    // A passkey made for another RP ID, or answered from another origin, is not added.
    let origin = ada.origin();
    for (origin, rp_id) in [
        (origin.as_str(), Some("elsewhere.example")),
        ("http://elsewhere.example", None),
    ] {
        let (status, response) = ada
            .passkey(&harness, "/auth/settings", |options| {
                FakeAuthenticator::create(options, origin, rp_id).1
            })
            .await;
        assert_eq!(status, 400, "{response}");
        assert!(response.contains("did not work here"), "{response}");
    }
    let mut authenticator = ada.add_passkey(&harness).await;
    // Signing in with it for another RP ID, or from another origin, is refused too.
    let mut elsewhere = Browser::at_localhost(&harness);
    authenticator.rp_id = "elsewhere.example".into();
    let (status, response) = elsewhere
        .sign_in_with_passkey(&harness, &mut authenticator)
        .await;
    assert_eq!(status, 400, "{response}");
    authenticator.rp_id = "localhost".into();
    let (status, response) = elsewhere
        .passkey(&harness, "/auth/login", |options| {
            authenticator.get(options, "http://elsewhere.example")
        })
        .await;
    assert_eq!(status, 400, "{response}");
    assert!(elsewhere.cookie.is_empty());
    // An answer is good for its page's ceremony once: posting it again is refused.
    let page = page_of(&elsewhere.get(&harness, "/auth/login").await);
    let answer = authenticator.get(&page.passkey.unwrap().options, &elsewhere.origin());
    let body = format!(
        "csrf={}&credential={}",
        portal::encode(&page.csrf),
        portal::encode(&answer)
    );
    let mut second = Browser::at_localhost(&harness);
    let (status, response) = elsewhere
        .request(&harness, "POST", "/auth/passkey/sign-in", &body)
        .await;
    assert_eq!(status, 303, "{response}");
    let (status, _) = second
        .request(&harness, "POST", "/auth/passkey/sign-in", &body)
        .await;
    assert_eq!(status, 400);
    assert!(second.cookie.is_empty());
    // Another page's form token carries no ceremony of this one's.
    let other = page_of(&second.get(&harness, "/auth/register").await);
    let body = format!(
        "csrf={}&credential={}",
        portal::encode(&other.csrf),
        portal::encode(
            &authenticator.get(
                &page_of(&second.get(&harness, "/auth/login").await)
                    .passkey
                    .unwrap()
                    .options,
                &second.origin()
            )
        )
    );
    let (status, response) = second
        .request(&harness, "POST", "/auth/passkey/sign-in", &body)
        .await;
    assert_eq!(status, 400, "{response}");
    assert!(second.cookie.is_empty());
}
