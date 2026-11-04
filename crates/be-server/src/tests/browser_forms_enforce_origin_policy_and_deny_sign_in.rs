use super::*;
#[tokio::test]
async fn browser_forms_enforce_origin_policy_and_deny_sign_in() {
    let harness = Harness::start().await;
    let mut app = harness.client().await;
    let (code, secret) = begin_session(&mut app).await;
    let finishing = client::account::finish_browser_login(&app.peer, code.clone(), secret.clone());
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
        matches!(finishing.await, Ok(Err(_))),
        "a sign-in denied in the browser was not refused"
    );
    assert!(matches!(
        app.call(|peer| client::account::finish_browser_login(peer, code, secret))
            .await,
        Err(_)
    ));
    assert_eq!(
        browser
            .request(&harness, "POST", "/auth/deny", &body)
            .await
            .0,
        400
    );
}
