use super::*;

/// Sessions, attempts, links, form tokens and a device's sign-in secret all come from one
/// generator: 256 bits from the operating system, in hex.
#[tokio::test]
async fn every_secret_the_server_hands_out_is_256_random_bits() {
    let is_secret = |text: &str| {
        text.len() == 64
            && text
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    };
    let harness = Harness::start().await;
    harness.member("ada@example.com").await;
    let mut browser = Browser::default();
    assert_eq!(browser.ask_code(&harness, "ada@example.com").await.0, 303);
    assert!(is_secret(&browser.attempt), "{}", browser.attempt);
    let (code, _) = harness.mailed_code("ada@example.com");
    assert_eq!(browser.enter_code(&harness, &code).await.0, 303);
    assert!(is_secret(&browser.cookie), "{}", browser.cookie);
    let form = csrf(&browser.get(&harness, "/auth/settings").await);
    assert!(is_secret(&form), "{form}");
    browser
        .submit(
            &harness,
            "/auth/settings",
            "/auth/settings/email",
            &[("email", "new@example.com")],
        )
        .await;
    let link = harness.mail_token("new@example.com");
    assert!(is_secret(&link), "{link}");

    let sessions = crate::browser_auth::Sessions::default();
    let (_, secret, _, _) = sessions.begin(std::net::IpAddr::from([192, 0, 2, 1]));
    assert!(is_secret(&secret), "{secret}");
    let secrets: std::collections::HashSet<String> = (0..64).map(|_| crate::new_secret()).collect();
    assert_eq!(secrets.len(), 64);
}
