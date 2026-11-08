use super::*;
use revisions::{Answered, Asked};

#[test]
fn a_restore_on_a_follower_keeps_the_state_it_replaced() {
    let harness = Harness::start();
    // History keeps a state per minute: each saved state below is a minute after the last.
    let start = be_block_commit::now_milliseconds();
    let at = |milliseconds: i64| be_block_commit::pin_now(Some(start + milliseconds));
    let minutes = |minutes: i64| at(minutes * be_block_commit::retention::MINUTE);
    minutes(0);
    let block = Uuid::new_v4();
    let mut outside = Outside::counter(&harness, block);
    outside.add(5);
    outside.seal();
    let peer = outside.peer();
    let first = outside
        .block_on(peer.summary(block))
        .expect("the counter is listed")
        .head
        .expect("the first state is saved");
    minutes(1);
    outside.add(2);
    outside.seal();
    let replaced = outside
        .block_on(peer.summary(block))
        .expect("the counter is listed")
        .head
        .expect("the second state is saved");
    assert_ne!(first, replaced);

    // The restore lands in the same minute as the state it replaces, a second later.
    at(be_block_commit::retention::MINUTE + 1_000);
    harness.connect();
    open(block, CounterContent::CONTENT_TYPE);
    wait_for_count(block, 7);
    assert!(outside.is_owner(), "the other peer owns the session");

    let (send, receive) = std::sync::mpsc::channel();
    revisions::request(
        block,
        Asked::Restore(*first.hash().as_bytes()),
        move |reply| {
            send.send(reply).unwrap();
        },
    );
    assert!(matches!(
        receive.recv().expect("the restore answers"),
        Answered::Restored(Ok(()))
    ));
    wait_for_count(block, 5);

    let kept: Vec<_> = outside
        .block_on(peer.history(block))
        .expect("the history reads back")
        .into_iter()
        .map(|entry| entry.commit)
        .collect();
    assert!(
        kept.contains(&replaced),
        "restoring on a follower let the state it replaced be pruned"
    );
}
