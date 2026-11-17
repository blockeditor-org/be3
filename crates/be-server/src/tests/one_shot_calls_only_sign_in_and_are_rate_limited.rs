use super::*;

/// A one-shot post carries only a call that signs in, and one address makes only so many.
#[tokio::test]
async fn one_shot_calls_only_sign_in_and_are_rate_limited() {
    let harness = Harness::start().await;
    let token = harness.register("member@example.com", "Member").await;

    let (sending, mut sent) = tokio::sync::mpsc::unbounded_channel();
    let peer = Peer::new(be_wasm_ops::Side::Guest, move |frame| {
        let _ = sending.send(frame);
    });
    let mut authenticating = be_block_protocol::client::server::authenticate(&peer, Some(token));
    let _ = poll(&mut authenticating);
    let call = sent.try_recv().expect("the call was started");
    let (status, _) = post_bytes(&harness, "/api/be/call", &framed(&[hello(BUILD), call])).await;
    assert_eq!(status, 403, "a one-shot call did more than sign in");

    for _ in 1..crate::http::MAX_CALLS_PER_ADDRESS {
        harness
            .once(|peer| begin_login(peer))
            .await
            .expect("a sign-in begins");
    }
    let (sending, mut sent) = tokio::sync::mpsc::unbounded_channel();
    let peer = Peer::new(be_wasm_ops::Side::Guest, move |frame| {
        let _ = sending.send(frame);
    });
    let mut beginning = client::server::device_login(&peer, DeviceLoginStep::Begin);
    let _ = poll(&mut beginning);
    let call = sent.try_recv().expect("the call was started");
    let (status, _) = post_bytes(&harness, "/api/be/call", &framed(&[hello(BUILD), call])).await;
    assert_eq!(
        status, 429,
        "an address made more one-shot calls than it may"
    );
}
