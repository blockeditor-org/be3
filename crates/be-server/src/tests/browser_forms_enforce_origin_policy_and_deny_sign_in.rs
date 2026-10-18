use super::*;

#[tokio::test]
async fn browser_forms_enforce_origin_policy_and_deny_sign_in() {
    let harness = Harness::start().await;
    let mut app = harness.client().await;
    let code = begin(&mut app).await;
    let csrf = form(&harness, &code).await;
    let body = format!("csrf={csrf}&code={code}&action=deny");
    assert_eq!(
        http(
            &harness,
            "POST",
            "/auth",
            &body,
            Some("https://unrelated.example")
        )
        .await
        .0,
        403
    );
    assert_eq!(post(&harness, &body).await.0, 200);
    assert!(matches!(
        app.notification().await,
        ServerMessage::BrowserLoginFinished {
            approved: false,
            ..
        }
    ));
    assert!(matches!(
        app.send(|request| ClientMessage::FinishBrowserLogin { request, code })
            .await,
        ServerMessage::Failed { .. }
    ));
}
