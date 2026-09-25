use super::*;

use be_client::Live;

#[test]
fn an_edit_made_across_a_takeover_is_kept() {
    let harness = Harness::start();
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("a test runtime starts");
    let block = Uuid::new_v4();
    let (outside, mut owner) = runtime.block_on(async {
        let outside = harness.outside_peer().await;
        outside
            .ensure::<CounterContent>(block, be_graph::BlockParent::Root)
            .await
            .expect("the counter is created");
        let owner = Live::<_, CounterContent>::join(std::sync::Arc::clone(&outside), block)
            .await
            .expect("the outside peer owns the session");
        (outside, owner)
    });
    assert!(owner.is_owner());

    harness.connect();
    open(block, CounterContent::CONTENT_TYPE);
    wait_for_count(block, 0);
    add(block, 10);
    runtime.block_on(async {
        while owner.content().root().value() != 10 {
            owner.wait().await.expect("the session is still up");
        }
    });
    wait_until("had its add sequenced", |shared| {
        counted(shared, block) == Some(10) && shared.unsealed == 0
    });

    runtime.block_on(async {
        owner
            .seal()
            .await
            .expect("the owner saves")
            .published()
            .expect("the owner's save was taken");
        outside
            .leave_session(block)
            .await
            .expect("the owner leaves");
    });
    add(block, 100);

    wait_until("saved what it typed across the takeover", |shared| {
        counted(shared, block) == Some(110) && shared.unsealed == 0
    });
    let stored = runtime.block_on(async {
        outside
            .open::<CounterContent>(block)
            .await
            .expect("the counter reads back")
            .expect("the counter has content")
            .root()
            .value()
    });
    assert_eq!(stored, 110);
}
