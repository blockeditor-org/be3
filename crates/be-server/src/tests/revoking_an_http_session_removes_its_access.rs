use super::a_client_speaks_the_protocol_over_http_posts::post;
use super::*;
#[tokio::test]
async fn revoking_an_http_session_removes_its_access() {
    let harness = Harness::start().await;
    let owner = harness.member("http-revoke@example.com").await;
    let mut stream = tokio::net::TcpStream::connect(harness.url.trim_start_matches("ws://"))
        .await
        .unwrap();
    let (_, session, _) = post(&mut stream, None, &[&hello(BUILD)]).await;
    let session = session.unwrap();
    let authenticate = encode(&ClientMessage::Authenticate {
        request: 1,
        token: owner.token.clone(),
    })
    .unwrap();
    assert_eq!(
        post(&mut stream, Some(&session), &[&authenticate]).await.0,
        200
    );
    let mut browser = Browser::default();
    browser
        .login(&harness, "http-revoke@example.com", "correct horse battery")
        .await;
    assert_eq!(
        browser
            .submit(&harness, "/auth/sessions", "/auth/sessions/others", &[])
            .await
            .0,
        303
    );
    let list = encode(&ClientMessage::ListOrganizations { request: 2 }).unwrap();
    assert_eq!(post(&mut stream, Some(&session), &[&list]).await.0, 404);
}
