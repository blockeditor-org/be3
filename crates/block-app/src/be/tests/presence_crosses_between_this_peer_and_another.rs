use super::*;

use be_client::Live;

const CURSOR: Uuid = Uuid::from_u128(0xc0);

#[test]
fn presence_crosses_between_this_peer_and_another() {
    let harness = Harness::start();
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("a test runtime starts");
    let block = Uuid::new_v4();
    let mut outside = runtime.block_on(async {
        let outside = harness.outside_peer().await;
        outside
            .ensure::<CounterContent>(block, be_graph::BlockParent::Root)
            .await
            .expect("the counter is created");
        Live::<_, CounterContent>::join(outside, block)
            .await
            .expect("the outside peer joins")
    });

    harness.connect();
    open(block, CounterContent::CONTENT_TYPE);
    wait_for_count(block, 0);

    show(block, CURSOR, Some(vec![1]));
    runtime.block_on(async {
        while !outside.presence().values().any(|value| *value == [1]) {
            outside.wait().await.expect("the session is still up");
        }
    });

    runtime.block_on(async {
        outside
            .set_presence(CURSOR, Some(vec![2]))
            .await
            .expect("the outside peer shows its cursor");
    });
    wait_until("saw the other peer's cursor", |shared| {
        shared.presence.get(&block).is_some_and(|(_, peers)| {
            peers
                .iter()
                .any(|peer| peer.kind == CURSOR && peer.value == [2])
        })
    });

    show(block, CURSOR, None);
    runtime.block_on(async {
        while !outside.presence().is_empty() {
            outside.wait().await.expect("the session is still up");
        }
    });
}
