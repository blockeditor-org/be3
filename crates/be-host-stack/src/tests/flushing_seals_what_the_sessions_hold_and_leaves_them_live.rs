use super::*;

#[test]
fn flushing_seals_what_the_sessions_hold_and_leaves_them_live() {
    let harness = Harness::start();
    harness.connect();
    let block = Uuid::new_v4();

    open(block, CounterContent::CONTENT_TYPE);
    wait_for_count(block, 0);
    add(block, 5);
    wait_for_count(block, 5);
    wait_until("journaled the edit without committing it", |shared| {
        shared.undurable == 0 && shared.unsealed == 1
    });
    assert_eq!(
        status().unsaved,
        0,
        "an edit in the device's journal is durable, only waiting for its commit"
    );

    flush();

    wait_until("committed the edit", |shared| shared.unsealed == 0);
    assert_eq!(status().unsaved, 0);
    assert!(status().connected);
    add(block, 1);
    wait_for_count(block, 6);
}
