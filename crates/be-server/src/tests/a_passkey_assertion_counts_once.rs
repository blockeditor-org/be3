use super::*;

#[tokio::test]
async fn a_passkey_assertion_counts_once() {
    let harness = Harness::start().await;
    harness.member("ada@example.com").await;
    let mut ada = Browser::at_localhost(&harness);
    ada.sign_in_by_code(&harness, "ada@example.com").await;
    let mut authenticator = ada.add_passkey(&harness).await;
    // Two sign-ins begun together, answered in turn: the later counter lands first, and the
    // earlier one, no longer past the stored counter, is refused as a clone's would be.
    let mut first = Browser::at_localhost(&harness);
    let mut second = Browser::at_localhost(&harness);
    let pages = [
        page_of(&first.get(&harness, "/auth/login").await),
        page_of(&second.get(&harness, "/auth/login").await),
    ];
    let answers: Vec<String> = pages
        .iter()
        .map(|page| authenticator.get(&page.passkey.as_ref().unwrap().options, &first.origin()))
        .collect();
    let post = |page: &be_server_portal::Page, answer: &str| {
        format!(
            "csrf={}&credential={}",
            portal::encode(&page.csrf),
            portal::encode(answer)
        )
    };
    let (status, response) = second
        .request(
            &harness,
            "POST",
            "/auth/passkey/sign-in",
            &post(&pages[1], &answers[1]),
        )
        .await;
    assert_eq!(status, 303, "{response}");
    let (status, response) = first
        .request(
            &harness,
            "POST",
            "/auth/passkey/sign-in",
            &post(&pages[0], &answers[0]),
        )
        .await;
    assert_eq!(status, 400, "{response}");
    assert!(response.contains("did not work here"), "{response}");
    assert!(first.cookie.is_empty());
    // An authenticator whose counter went back is refused the same way.
    authenticator.counter = 1;
    let (status, _) = first
        .sign_in_with_passkey(&harness, &mut authenticator)
        .await;
    assert_eq!(status, 400);
}
