use super::a_client_speaks_the_protocol_over_http_posts::{Posted, post};
use super::*;
#[tokio::test]
async fn revoking_an_http_session_removes_its_access() {
    let harness = Harness::start().await;
    let owner = harness.member("http-revoke@example.com").await;
    let mut stream = tokio::net::TcpStream::connect(harness.address)
        .await
        .unwrap();
    let (_, session, _) = post(&mut stream, None, &[&hello(BUILD)]).await;
    let session = session.unwrap();
    let mut posted = Posted::new();
    let _authenticating =
        be_block_protocol::client::server::authenticate(&posted.peer, Some(owner.token.clone()));
    let sent = posted.frames();
    let (status, _, frames) = post(&mut stream, Some(&session), &[&sent[0]]).await;
    assert_eq!(status, 200);
    posted.heard(&frames);
    let mut browser = Browser::default();
    browser
        .sign_in_by_code(&harness, "http-revoke@example.com")
        .await;
    browser.prove(&harness, "http-revoke@example.com").await;
    assert_eq!(
        browser
            .submit(&harness, "/auth/sessions", "/auth/sessions/others", &[])
            .await
            .0,
        303
    );
    let _listed = posted
        .peer
        .read::<Vec<be_block_protocol::Organization>>(be_block_protocol::Query::Organizations);
    let list = posted.frames();
    assert_eq!(post(&mut stream, Some(&session), &[&list[0]]).await.0, 404);
}
