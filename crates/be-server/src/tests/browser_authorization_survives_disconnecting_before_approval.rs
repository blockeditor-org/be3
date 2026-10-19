use super::*;

#[tokio::test]
async fn browser_authorization_survives_disconnecting_before_approval() {
    let harness = Harness::start().await;
    let mut app = harness.client().await;
    let (code, secret) = begin_session(&mut app).await;
    app.socket.close(None).await.unwrap();
    while app.socket.next().await.is_some() {}
    drop(app);
    let csrf = form(&harness, &code).await;
    let body = format!(
        "csrf={csrf}&code={code}&action=register&email=offline%40example.com&display_name=Offline&password=correct+horse+battery"
    );
    assert_eq!(post(&harness, &body).await.0, 200);
    let mut app = harness.client().await;
    assert!(matches!(
        app.send(|request| ClientMessage::FinishBrowserLogin { request, code, secret }).await,
        ServerMessage::Authenticated { email, .. } if email == "offline@example.com"
    ));
}
