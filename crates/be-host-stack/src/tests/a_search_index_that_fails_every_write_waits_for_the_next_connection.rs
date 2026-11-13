use super::*;

#[test]
fn a_search_index_that_fails_every_write_waits_for_the_next_connection() {
    let (index, block, whole) = (Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4());
    let now = Duration::ZERO;
    let mut search = Searching::default();

    // Queued again after each failure, those read whole to be read whole again.
    assert!(!search.write_failed(index, vec![block, whole], vec![whole], now));
    let mut queued = search.take_queued();
    queued.sort();
    let mut expected = vec![block, whole];
    expected.sort();
    assert_eq!(queued, expected);
    assert!(search.reads_whole(whole));
    assert!(!search.reads_whole(block));
    assert!(!search.write_failed(index, vec![block], Vec::new(), now));
    assert_eq!(search.take_queued(), [block]);

    // The third in a row sets the batch aside, for the host to say why. Nothing is busy, but
    // searches are not complete (`indexed` is what an answer's `complete` reads).
    assert!(search.write_failed(index, vec![block], Vec::new(), now));
    assert!(search.take_queued().is_empty());
    assert!(!search.busy(false));
    assert!(!search.indexed());

    // The next connection queues it again, and a failure counts from one.
    search.reconnected();
    assert_eq!(search.take_queued(), [block]);
    assert!(search.indexed());
    assert!(!search.write_failed(index, vec![block], Vec::new(), now));
    search.take_queued();

    // A batch written starts the count over.
    assert!(!search.write_failed(index, vec![block], Vec::new(), now));
    search.wrote(index);
    assert!(!search.write_failed(index, vec![block], Vec::new(), now));
    assert!(!search.write_failed(index, vec![block], Vec::new(), now));
    search.take_queued();
}
