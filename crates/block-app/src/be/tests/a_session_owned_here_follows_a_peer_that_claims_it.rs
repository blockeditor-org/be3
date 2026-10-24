use super::*;

use be_client::Live;

#[test]
fn a_session_owned_here_follows_a_peer_that_claims_it() {
    let harness = Harness::lapsing();
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("a test runtime starts");
    let block = Uuid::new_v4();
    let outside = runtime.block_on(async {
        let outside = harness.outside_peer().await;
        outside
            .ensure::<CounterContent>(block, be_graph::BlockParent::Root)
            .await
            .expect("the counter is created");
        outside
    });

    harness.connect();
    open(block, CounterContent::CONTENT_TYPE);
    wait_for_count(block, 0);
    let mut claimant = runtime.block_on(async {
        let mut live = Live::<_, CounterContent>::join(std::sync::Arc::clone(&outside), block)
            .await
            .expect("the outside peer joins");
        assert!(!live.is_owner(), "this peer opened the block first");
        loop {
            live.poll().await.expect("the session is still up");
            if live.claim().await.expect("the claim is answered") {
                return live;
            }
            live.wait().await.expect("the session is still up");
        }
    });
    let before = claimant.head();

    runtime.block_on(async {
        claimant
            .edit(Counter::add(5))
            .await
            .expect("the new owner edits");
    });
    wait_for_count(block, 5);

    add(block, 10);
    runtime.block_on(async {
        while claimant.content().root().value() != 15 {
            claimant.wait().await.expect("the session is still up");
        }
    });
    wait_until("had its add sequenced by the new owner", |shared| {
        counted(shared, block) == Some(15) && shared.unsealed == 0 && shared.owing == 0
    });
    let remote = runtime.block_on(async {
        outside
            .remote_head(block)
            .await
            .expect("the head reads back")
    });
    assert_eq!(
        remote, before,
        "the old owner saved after giving up the session"
    );

    let stored = runtime.block_on(async {
        claimant
            .seal()
            .await
            .expect("the new owner saves")
            .published()
            .expect("the new owner's save was taken");
        outside
            .open::<CounterContent>(block)
            .await
            .expect("the counter reads back")
            .expect("the counter has content")
            .root()
            .value()
    });
    assert_eq!(stored, 15);
}
