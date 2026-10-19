use super::*;
#[tokio::test]
async fn a_closed_server_also_refuses_browser_registration_and_login() {
    let harness = Harness::configured(ServerConfig::default()).await;
    let mut browser = Browser::default();
    for action in ["login", "register", "forgot", "verify", "reset", "approve"] {
        let (status, _) = browser
            .submit(
                &harness,
                "/auth",
                &format!("/auth/{action}"),
                &[
                    ("email", "ada@example.com"),
                    ("password", "correct horse battery"),
                    ("confirmation", "correct horse battery"),
                ],
            )
            .await;
        assert_eq!(status, 400, "{action}");
    }
    assert!(harness.mail.0.lock().unwrap().is_empty());
}
