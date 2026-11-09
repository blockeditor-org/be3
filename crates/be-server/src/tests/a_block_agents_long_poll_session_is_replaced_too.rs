use super::a_client_speaks_the_protocol_over_http_posts::{Posted, post};
use super::*;

#[tokio::test]
async fn a_block_agents_long_poll_session_is_replaced_too() {
    let harness = Harness::start().await;
    let mut owner = harness.member("owner@example.com").await;
    let organization = owner.organization("team").await;
    let terminal = owner.create_block(BlockParent::Root).await;
    let (_, token) = owner.block_agent(organization, terminal).await.unwrap();

    let mut stream = tokio::net::TcpStream::connect(harness.address)
        .await
        .unwrap();
    let (_, session, _) = post(&mut stream, None, &[&hello(BUILD)]).await;
    let session = session.unwrap();
    let mut posted = Posted::new();
    let _authenticating = client::account::authenticate(&posted.peer, Some(token.clone()));
    let sent = posted.frames();
    let (status, _, frames) = post(&mut stream, Some(&session), &[&sent[0]]).await;
    assert_eq!(status, 200);
    posted.heard(&frames);

    let mut socket = harness.second_connection(&token, organization).await;
    let _listed = client::organizations::list_organizations(&posted.peer);
    let list = posted.frames();
    assert_eq!(
        post(&mut stream, Some(&session), &[&list[0]]).await.0,
        404,
        "the long-poll session outlived the agent's newer connection"
    );
    socket.round_trip().await;

    harness.stop().await;
}
