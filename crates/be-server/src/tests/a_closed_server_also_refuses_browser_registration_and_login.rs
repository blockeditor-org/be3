use super::*;
#[tokio::test]
async fn a_closed_server_also_refuses_browser_registration_and_login() {
    let harness = Harness::configured(ServerConfig::default()).await;
    let mut browser = Browser::default();
    for action in ["code/send", "register", "enter", "verify", "approve"] {
        let (status, _) = browser
            .submit(
                &harness,
                "/auth",
                &format!("/auth/{action}"),
                &[
                    ("email", "ada@example.com"),
                    ("display_name", "Ada"),
                    ("email_code", "23456789"),
                ],
            )
            .await;
        assert_eq!(status, 400, "{action}");
    }
    assert!(harness.mail.0.lock().unwrap().is_empty());
}
