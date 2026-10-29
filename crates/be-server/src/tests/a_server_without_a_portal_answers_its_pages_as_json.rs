use super::*;

#[tokio::test]
async fn a_server_without_a_portal_answers_its_pages_as_json() {
    let harness = Harness::configured(ServerConfig::OPEN).await;
    let (status, response) = http(&harness, "GET", "/auth/login", "", None).await;
    assert_eq!(status, 200);
    assert!(response.contains("Content-Type: application/json"));
    let page = page_of(&response);
    assert_eq!(page.screen, be_portal::Screen::SignIn);
    let mut browser = Browser::default();
    let (status, response) = browser
        .submit(
            &harness,
            "/auth/login",
            "/auth/login",
            &[
                ("email", "nobody@example.com"),
                ("password", "a password that is wrong"),
            ],
        )
        .await;
    assert_eq!(status, 400);
    let page = page_of(&response);
    assert!(page.problem.is_some());
    assert!(!response.contains("a password that is wrong"));
}
