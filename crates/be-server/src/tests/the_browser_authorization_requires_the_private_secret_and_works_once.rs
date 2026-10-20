use super::*;
#[tokio::test]
async fn the_browser_authorization_requires_the_private_secret_and_works_once() {
    let harness = Harness::start().await;
    let mut app = harness.client().await;
    let mut other = harness.client().await;
    let (code, secret) = begin_session(&mut app).await;
    let mut browser = Browser::default();
    let (status, page) = browser
        .submit(
            &harness,
            "/auth/register",
            "/auth/register",
            &[
                ("code", &code),
                ("email", "ada@example.com"),
                ("display_name", "Ada"),
                ("password", "correct horse battery"),
                ("confirmation", "correct horse battery"),
            ],
        )
        .await;
    assert_eq!(status, 303, "{page}");
    assert!(!page.contains("correct horse"));
    assert!(matches!(
        app.send(|request| ClientMessage::FinishBrowserLogin {
            request,
            code: code.clone(),
            secret: secret.clone()
        })
        .await,
        ServerMessage::Ok { .. }
    ));
    let token = harness.mail_token("ada@example.com");
    let path = format!("/auth/verify?token={token}");
    assert_eq!(
        browser
            .submit(&harness, &path, "/auth/verify", &[("token", &token)])
            .await
            .0,
        303
    );
    assert!(matches!(
        app.send(|request| ClientMessage::FinishBrowserLogin {
            request,
            code: code.clone(),
            secret: secret.clone()
        })
        .await,
        ServerMessage::Ok { .. }
    ));
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
    assert!(matches!(
        app.send(|request| ClientMessage::FinishBrowserLogin {
            request,
            code,
            secret
        })
        .await,
        ServerMessage::Failed { .. }
    ));
    assert!(matches!(
        other
            .send(|request| ClientMessage::Authenticate { request, token })
            .await,
        ServerMessage::Authenticated { .. }
    ));
}
