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
    assert_eq!(status().unsealed, 1);

    flush();

    assert_eq!(status().unsealed, 0);
    assert!(status().connected);
    add(block, 1);
    wait_for_count(block, 6);
}
