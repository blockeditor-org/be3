use super::*;

#[test]
fn an_idle_peer_never_wakes_its_worker() {
    let harness = Harness::start();
    harness.connect();
    let block = Uuid::new_v4();

    open(block, CounterContent::CONTENT_TYPE);
    wait_for_count(block, 0);
    flush();
    let settled = status().wakes;

    let woke = wait_for(QUIET, |shared| {
        (shared.wakes != settled).then_some(shared.wakes)
    });

    assert_eq!(
        woke, None,
        "the worker woke {woke:?} times without being asked"
    );
}
