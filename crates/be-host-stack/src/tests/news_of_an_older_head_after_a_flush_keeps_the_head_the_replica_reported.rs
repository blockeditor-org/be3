use super::*;

#[test]
fn news_of_an_older_head_after_a_flush_keeps_the_head_the_replica_reported() {
    let harness = Harness::start();
    harness.connect();
    let block = Uuid::new_v4();
    open(block, CounterContent::CONTENT_TYPE);
    wait_for_count(block, 0);
    add(block, 5);
    wait_for_count(block, 5);
    flush();
    let first = node(block).unwrap().head.unwrap();
    add(block, 2);
    wait_for_count(block, 7);
    flush();
    let second = node(block).unwrap().head.unwrap();
    assert_ne!(first, second);

    hear_head(block, first);
    flush();
    assert_eq!(node(block).unwrap().head, Some(second));
}
