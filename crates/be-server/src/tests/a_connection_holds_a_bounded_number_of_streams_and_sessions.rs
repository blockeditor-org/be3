use super::*;
use crate::limits::*;

#[tokio::test]
async fn a_connection_holds_a_bounded_number_of_streams_and_sessions() {
    let harness = Harness::start().await;
    let mut member = harness.member("member@example.com").await;
    member.personal().await;
    let block = member.create_block(BlockParent::Root).await;

    let opened = member.heard.len();
    let mut streams: Vec<_> = (opened..MAX_STREAMS_PER_CONNECTION)
        .map(|_| client::account::paired(&member.peer))
        .collect();
    member.round_trip().await;
    let mut refused = client::account::paired(&member.peer);
    assert!(refused.recv().await.is_none());
    assert_eq!(refused.ended(), Some(Err(be_wasm_ops::OpError::Denied)));
    let mut context = std::task::Context::from_waker(std::task::Waker::noop());
    assert!(streams[0].poll_recv(&mut context).is_pending());
    drop(streams);
    member.round_trip().await;

    let held: Vec<_> = (0..MAX_SESSIONS_PER_CONNECTION)
        .map(|_| client::sessions::Session::new(&member.peer, block))
        .collect();
    let session = client::sessions::Session::new(&member.peer, block);
    let mut events = session.join();
    assert!(
        events.recv().await.is_none(),
        "a session past the limit was joined"
    );
    drop(held);
    member.round_trip().await;
    member.join_session(block).await;

    let relayed = member
        .relay(block, None, vec![0; MAX_RELAY_BYTES + 1])
        .await;
    assert!(
        refused_as(&relayed, ErrorCode::InvalidRequest),
        "{relayed:?}"
    );
    let paired = member
        .call(|peer| client::account::pair(peer, None, vec![0; MAX_PAIRING_BYTES + 1]))
        .await;
    assert!(refused_as(&paired, ErrorCode::InvalidRequest), "{paired:?}");

    harness.stop().await;
}
