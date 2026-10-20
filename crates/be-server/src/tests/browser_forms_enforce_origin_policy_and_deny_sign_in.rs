use super::*;
#[tokio::test]
async fn browser_forms_enforce_origin_policy_and_deny_sign_in() {
    let harness = Harness::start().await;
    let mut app = harness.client().await;
    let (code, secret) = begin_session(&mut app).await;
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
    assert!(matches!(
        app.notification().await,
        ServerMessage::BrowserLoginFinished {
            approved: false,
            ..
        }
    ));
    assert!(matches!(
        app.send(|request| ClientMessage::FinishBrowserLogin {
            request,
            code,
            secret
        })
        .await,
        ServerMessage::Failed { .. }
    ));
    assert_eq!(
        browser
            .request(&harness, "POST", "/auth/deny", &body)
            .await
            .0,
        400
    );
}
