use super::*;
use be_block_session::Membership;

#[tokio::test]
async fn a_relay_to_a_client_that_left_the_session_is_dropped_without_an_error() {
    tokio::time::timeout(std::time::Duration::from_secs(40), body())
        .await
        .expect("the test hung");
}

async fn body() {
    let harness = Harness::start().await;
    let mut owner = harness.client().await;
    let account = owner.register("owner@example.com").await;
    let organization = owner.organization("notes").await;
    let block = owner.create_block(BlockParent::Root).await;
    let author = Author::new(account);

    let (_, mut state) = owner.join_session(block).await;
    let mut peer = harness
        .second_connection(&owner.token.clone(), organization)
        .await;
    let (peer_id, _) = peer.join_session(block).await;
    let index = state.index + 1;
    owner.membership_until(block, &mut state, index).await;
    assert!(state.has(peer_id), "{state:?}");

    // The peer leaves (dropping its session), and the owner, which cannot know yet, answers
    // it: the answer goes nowhere and is not an error.
    peer.sessions.remove(&block);
    let index = state.index + 1;
    owner.membership_until(block, &mut state, index).await;
    assert!(!state.has(peer_id), "the peer never left the session");
    let sealed = author.commits.vault().seal(KEY, b"sequenced").unwrap();
    let response = owner.relay(block, Some(peer_id), sealed.clone()).await;
    assert!(matches!(response, Ok(())), "{response:?}");

    // A client the session never had is the same.
    let response = owner.relay(block, Some(9_999), sealed).await;
    assert!(matches!(response, Ok(())), "{response:?}");

    harness.stop().await;
}
