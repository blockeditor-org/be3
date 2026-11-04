use super::*;
#[tokio::test]
async fn browser_forms_enforce_origin_policy_and_deny_sign_in() {
    let harness = Harness::configured(ServerConfig {
        sign_in_poll_interval: Duration::ZERO,
        ..ServerConfig::OPEN
    })
    .await;
    let (code, secret) = begin_session(&harness).await;
    let page = http(&harness, "GET", "/auth", "", None).await.1;
    let body = format!("csrf={}&code={code}", csrf(&page));
    assert_eq!(
        http(
            &harness,
            "POST",
            "/auth/deny",
            &body,
            Some("https://unrelated.example")
        )
        .await
        .0,
        403
    );
    let mut browser = Browser::default();
    assert_eq!(
        browser
            .request(&harness, "POST", "/auth/deny", &body)
            .await
            .0,
        303
    );
    assert!(
        matches!(
            poll_login(&harness, &code, &secret).await,
            Ok(be_block_protocol::BrowserLoginState::Denied)
        ),
        "a sign-in denied in the browser was not refused"
    );
    assert!(matches!(
        poll_login(&harness, &code, &secret).await,
        Ok(be_block_protocol::BrowserLoginState::Expired)
    ));
    assert_eq!(
        browser
            .request(&harness, "POST", "/auth/deny", &body)
            .await
            .0,
        400
    );
}
