use super::*;

#[tokio::test]
async fn the_browser_authorization_requires_the_private_secret_and_works_once() {
    let harness = Harness::start().await;
    let mut app = harness.client().await;
    let mut other = harness.client().await;
    let (code, secret) = begin_session(&mut app).await;
    assert!(matches!(
        app.send(|request| ClientMessage::FinishBrowserLogin {
            request,
            code: code.clone(),
            secret: secret.clone()
        })
        .await,
        ServerMessage::Ok { .. }
    ));
    let csrf = form(&harness, &code).await;
    let body = format!(
        "csrf={csrf}&code={code}&action=register&email=ada%40example.com&display_name=Ada&password=correct+horse+battery"
    );
    let (status, page) = post(&harness, &body).await;
    assert_eq!(status, 200, "{page}");
    assert!(!page.contains("correct horse"));
    assert!(matches!(
        app.notification().await,
        ServerMessage::BrowserLoginFinished { approved: true, .. }
    ));
    assert!(matches!(
        other
            .send(|request| ClientMessage::FinishBrowserLogin {
                request,
                code: code.clone(),
                secret: "wrong secret".into()
            })
            .await,
        ServerMessage::Failed { .. }
    ));
    drop(app);
    let mut app = harness.client().await;
    let session = app
        .send(|request| ClientMessage::FinishBrowserLogin {
            request,
            code: code.clone(),
            secret: secret.clone(),
        })
        .await;
    let ServerMessage::Authenticated { token, email, .. } = session else {
        panic!("{session:?}")
    };
    assert_eq!(email, "ada@example.com");
    assert!(!page.contains(&token));
    assert!(matches!(
        app.send(|request| ClientMessage::FinishBrowserLogin {
            request,
            code: code.clone(),
            secret: secret.clone()
        })
        .await,
        ServerMessage::Failed { .. }
    ));
    assert_eq!(post(&harness, &body).await.0, 400);
    assert!(matches!(
        other
            .send(|request| ClientMessage::Authenticate { request, token })
            .await,
        ServerMessage::Authenticated { .. }
    ));
    let (code, _) = begin_session(&mut app).await;
    let csrf = form(&harness, &code).await;
    assert_eq!(post(&harness, &format!("csrf={csrf}&code={code}&action=login&email=ada%40example.com&password=correct+horse+battery")).await.0, 200);
    assert!(matches!(
        app.notification().await,
        ServerMessage::BrowserLoginFinished { approved: true, .. }
    ));
}
