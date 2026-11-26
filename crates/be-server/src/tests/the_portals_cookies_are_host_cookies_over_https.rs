use super::*;

#[tokio::test]
async fn the_portals_cookies_are_host_cookies_over_https() {
    let harness = Harness::configured(ServerConfig {
        auth_origin: Some("https://portal.example".into()),
        ..ServerConfig::OPEN
    })
    .await;
    harness.member("ada@example.com").await;
    let (status, page) = http(&harness, "GET", "/auth/login", "", None).await;
    assert_eq!(status, 200, "{page}");
    let body = format!(
        "csrf={}&email={}",
        csrf(&page),
        portal::encode("ada@example.com")
    );
    let (status, response) = http(
        &harness,
        "POST",
        "/auth/code/send",
        &body,
        Some("https://portal.example"),
    )
    .await;
    assert_eq!(status, 303, "{response}");
    // Over HTTPS a `__Host-` cookie, for the whole origin, which no sibling subdomain can set
    // or shadow; no bare one.
    let attempt = response
        .lines()
        .find(|line| line.starts_with("Set-Cookie: "))
        .unwrap_or_else(|| panic!("no cookie in {response}"));
    assert!(
        attempt.starts_with("Set-Cookie: __Host-be3-attempt="),
        "{attempt}"
    );
    for part in ["Path=/;", "HttpOnly", "SameSite=Lax", "Secure"] {
        assert!(attempt.contains(part), "{part} missing from {attempt}");
    }
    assert!(!attempt.contains("Domain"), "{attempt}");
    assert!(!response.contains("Set-Cookie: be3-"), "{response}");
}
