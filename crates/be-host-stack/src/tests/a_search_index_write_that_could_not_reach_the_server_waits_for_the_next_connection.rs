use super::*;

#[test]
fn a_search_index_write_that_could_not_reach_the_server_waits_for_the_next_connection() {
    let (index, block, whole) = (Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4());
    let mut search = Searching::default();

    // Set aside at once, with no delay to wait out and no failure counted.
    search.write_unreachable(index, vec![block, whole], vec![whole]);
    assert!(search.take_queued().is_empty());
    assert!(search.reads_whole(whole));
    assert_eq!(search.moving_due(), None);
    search.release_failing(Some(Duration::from_secs(60 * 60 * 24 * 365)));
    assert!(search.take_queued().is_empty(), "released on a delay");
    assert!(!search.indexed());

    // The next connection queues it again.
    search.reconnected();
    let mut queued = search.take_queued();
    queued.sort();
    let mut expected = vec![block, whole];
    expected.sort();
    assert_eq!(queued, expected);
    assert!(search.indexed());

    // No failure was counted: a real one is still the first of a row.
    assert!(!search.write_failed(index, vec![block], Vec::new(), Duration::ZERO));
    assert!(!search.write_failed(index, vec![block], Vec::new(), Duration::ZERO));
}
