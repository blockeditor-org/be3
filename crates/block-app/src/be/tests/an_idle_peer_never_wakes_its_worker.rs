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

    std::thread::sleep(Duration::from_secs(2));

    assert_eq!(status().wakes, settled);
}
