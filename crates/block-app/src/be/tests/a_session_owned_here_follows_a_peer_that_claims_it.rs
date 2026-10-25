use super::*;

#[test]
fn a_session_owned_here_follows_a_peer_that_claims_it() {
    let harness = Harness::lapsing();
    let block = Uuid::new_v4();
    runtime().block_on(async {
        harness
            .outside_peer()
            .await
            .ensure::<CounterContent>(block, be_graph::BlockParent::Root)
            .await
            .expect("the counter is created");
    });

    harness.connect();
    open(block, CounterContent::CONTENT_TYPE);
    wait_for_count(block, 0);
    let mut outside = Outside::joining(&harness, block);
    assert!(!outside.is_owner(), "this peer opened the block first");
    outside.claim();

    outside.add(5);
    wait_for_count(block, 5);
    add(block, 10);
    outside.until("had this peer's add", |seen| seen.count == Some(15));
    wait_until("had its add sequenced by the new owner", |shared| {
        counted(shared, block) == Some(15) && shared.unsealed == 0 && shared.owing == 0
    });

    outside.seal();
    let peer = outside.peer();
    let (stored, history) = outside.block_on(async {
        let stored = peer
            .open::<CounterContent>(block)
            .await
            .expect("the counter reads back")
            .expect("the counter has content")
            .root()
            .value();
        let history = peer.history(block).await.expect("the history reads back");
        (stored, history)
    });
    assert_eq!(stored, 15);
    assert!(
        history.iter().all(|entry| entry.parents.len() <= 1),
        "the old owner saved after giving up the session: {history:?}"
    );
}
