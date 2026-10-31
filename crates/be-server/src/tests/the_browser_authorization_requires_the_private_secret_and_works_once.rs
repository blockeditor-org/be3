use super::*;
#[tokio::test]
async fn the_browser_authorization_requires_the_private_secret_and_works_once() {
    let harness = Harness::start().await;
    let mut app = harness.client().await;
    let mut other = harness.client().await;
    let (code, secret) = begin_session(&mut app).await;
    let mut finishing =
        client::account::finish_browser_login(&app.peer, code.clone(), secret.clone());
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
    app.round_trip().await;
    assert!(
        poll(&mut finishing).is_pending(),
        "a sign-in finished before the browser approved it"
    );
    let token = harness.mail_token("ada@example.com");
    let path = format!("/auth/verify?token={token}");
    assert_eq!(
        browser
            .submit(&harness, &path, "/auth/verify", &[("token", &token)])
            .await
            .0,
        303
    );
    app.round_trip().await;
    assert!(
        poll(&mut finishing).is_pending(),
        "a sign-in finished before the browser approved it"
    );
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
        other
            .call(|peer| client::account::finish_browser_login(
                peer,
                code.clone(),
                "wrong secret".into()
            ))
            .await,
        Err(_)
    ));
    let session = finishing.await.expect("the connection stayed open");
    let Ok(Authenticated { token, email, .. }) = session else {
        panic!("{session:?}")
    };
    assert_eq!(email, "ada@example.com");
    assert!(matches!(
        app.call(|peer| client::account::finish_browser_login(peer, code, secret))
            .await,
        Err(_)
    ));
    assert!(matches!(
        other
            .call(|peer| client::account::authenticate(peer, token))
            .await,
        Ok(_)
    ));
}
