use super::*;

/// What the server's greeting says to a socket that greeted it with `token`, and the client over
/// the socket once it was read.
async fn welcome(harness: &Harness, token: Option<&str>) -> (be_block_protocol::Welcome, TestClient) {
    let mut socket = greet(&harness.socket(), BUILD, token).await;
    let frame = next_binary(&mut socket).await.expect("the server greets");
    let hello = read_hello(&frame).expect("the server greets");
    assert_eq!(hello.build, BUILD);
    let welcome = hello
        .said::<be_block_protocol::Welcome>()
        .expect("the server's greeting says who signed in");
    (welcome, TestClient::over(socket))
}

/// A connection signs in with its greeting: one whose greeting carries a bad token, or none and
/// no Authorization header, is told why and closed before any call, and logging out closes one.
#[tokio::test]
async fn a_greeting_that_signs_nothing_in_is_closed_before_any_call() {
    let harness = Harness::start().await;
    let token = harness.register("signed@example.com", "Signed").await;
    let mut signed = harness.signed_in(&token).await;

    for (token, refused) in [
        (Some("not a token"), ErrorCode::InvalidCredentials),
        (None, ErrorCode::NotAuthenticated),
    ] {
        let (welcome, mut closed) = welcome(&harness, token).await;
        assert!(
            matches!(&welcome, Err(Error { code, .. }) if *code == refused),
            "{welcome:?}"
        );
        let listed = closed
            .peer
            .read::<Vec<be_block_protocol::Organization>>(be_block_protocol::Query::Organizations)
            .await;
        assert!(
            !matches!(listed, Ok(Ok(_))),
            "a connection its greeting did not sign in was answered: {listed:?}"
        );
        closed.ended().await;
    }

    let mut logging_out = harness.member("leaving@example.com").await;
    let response = logging_out
        .call(|peer| peer.write::<()>(be_block_protocol::Op::RevokeToken(None)))
        .await;
    assert!(matches!(response, Ok(())), "{response:?}");
    logging_out.ended().await;
    let token = logging_out.token.clone();
    assert!(
        matches!(harness.sign_in(&token).await, Err(Error { code, .. }) if code == ErrorCode::InvalidCredentials),
        "a token logged out still signs a greeting in"
    );

    signed.round_trip().await;
    harness.stop().await;
}
