use super::*;

#[tokio::test]
async fn a_closed_server_also_refuses_browser_registration_and_login() {
    let harness = Harness::configured(ServerConfig::default()).await;
    let mut app = harness.client().await;
    let code = begin(&mut app).await;
    let csrf = form(&harness, &code).await;
    let (status, page) = post(&harness, &format!("csrf={csrf}&code={code}&action=register&email=ada%40example.com&display_name=Ada&password=correct+horse+battery")).await;
    assert_eq!(status, 400);
    assert!(!page.contains("value=login"));
    assert!(!page.contains("value=register"));
    let csrf = form(&harness, &code).await;
    assert_eq!(post(&harness, &format!("csrf={csrf}&code={code}&action=login&email=ada%40example.com&password=correct+horse+battery")).await.0, 400);
}
