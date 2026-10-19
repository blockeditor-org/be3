use super::*;
#[tokio::test]
async fn browser_authorization_survives_disconnecting_before_approval() {
    let harness = Harness::start().await;
    harness.member("offline@example.com").await;
    let mut app = harness.client().await;
    let (code, secret) = begin_session(&mut app).await;
    app.socket.close(None).await.unwrap();
    while app.socket.next().await.is_some() {}
    drop(app);
    let mut browser = Browser::default();
    browser
        .login(&harness, "offline@example.com", "correct horse battery")
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
    let mut app = harness.client().await;
    assert!(
        matches!(app.send(|request| ClientMessage::FinishBrowserLogin { request, code, secret }).await, ServerMessage::Authenticated { email, .. } if email == "offline@example.com")
    );
}
