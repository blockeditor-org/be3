use super::*;
use crate::limits::*;

#[tokio::test]
async fn a_connection_holds_one_events_stream_and_a_bounded_number_of_sessions() {
    let harness = Harness::start().await;
    let mut member = harness.member("member@example.com").await;
    member.personal().await;

    let mut first = member.heard.take().expect("signing in opened the events");
    let mut second = client::server::events(&member.peer);
    member.round_trip().await;
    assert!(first.recv().await.is_none());
    assert_eq!(
        first.ended(),
        Some(Err(be_wasm_ops::OpError::Canceled)),
        "a second events stream replaces the first"
    );
    let mut context = std::task::Context::from_waker(std::task::Waker::noop());
    assert!(second.poll_recv(&mut context).is_pending());
    member.heard = Some(second);

    let mut blocks = Vec::new();
    while blocks.len() <= MAX_SESSIONS_PER_CONNECTION {
        let mut ops = Vec::new();
        for _ in 0..64 {
            let (block, made) = member.new_block_op(BlockParent::Root).await;
            blocks.push(block);
            ops.extend(made);
        }
        let made = member.call(|peer| peer.writes(ops)).await;
        assert!(made.is_ok(), "{made:?}");
    }
    let joins: Vec<_> = blocks[..MAX_SESSIONS_PER_CONNECTION]
        .iter()
        .map(|&block| {
            member.peer.ask::<be_block_protocol::JoinedSession>(
                Query::Session {
                    block,
                    tag: TYPE_BUILD,
                },
                true,
            )
        })
        .collect();
    for joined in futures_util::future::join_all(joins).await {
        assert!(matches!(joined, Ok(Ok(_))), "{joined:?}");
    }
    let past = blocks[MAX_SESSIONS_PER_CONNECTION];
    let refused = member.try_join(past).await;
    assert!(
        refused_as(&refused, ErrorCode::OverQuota),
        "a session past the limit was joined: {refused:?}"
    );
    let left = member
        .call(|peer| peer.unfollow(Topic::Session(blocks[0])))
        .await;
    assert!(left.is_ok(), "{left:?}");
    member.join_session(past).await;

    let relayed = member.relay(past, None, vec![0; MAX_RELAY_BYTES + 1]).await;
    assert!(
        refused_as(&relayed, ErrorCode::InvalidRequest),
        "{relayed:?}"
    );
    let paired = member
        .call(|peer| peer.send(Target::Devices(None), vec![0; MAX_PAIRING_BYTES + 1]))
        .await;
    assert!(refused_as(&paired, ErrorCode::InvalidRequest), "{paired:?}");

    harness.stop().await;
}
