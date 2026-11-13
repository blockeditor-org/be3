use super::*;

#[test]
fn a_search_index_set_aside_is_tried_again_after_a_growing_delay() {
    let (index, block) = (Uuid::new_v4(), Uuid::new_v4());
    let second = Duration::from_secs(1);
    let mut search = Searching::default();
    let set_aside = |search: &mut Searching, now: Duration| {
        for _ in 0..3 {
            search.write_failed(index, vec![block], Vec::new(), now);
            search.take_queued();
        }
    };

    // Due half a minute after it was set aside, which the worker wakes for, without a
    // connection.
    set_aside(&mut search, Duration::ZERO);
    assert_eq!(search.moving_due(), Some(30 * second));
    search.release_failing(Some(29 * second));
    assert!(search.take_queued().is_empty());
    search.release_failing(Some(30 * second));
    assert_eq!(search.take_queued(), [block]);
    assert_eq!(search.moving_due(), None);

    // Set aside again in a row, it waits twice as long; never more than five minutes.
    set_aside(&mut search, Duration::ZERO);
    assert_eq!(search.moving_due(), Some(60 * second));
    for _ in 0..8 {
        search.release_failing(None);
        search.take_queued();
        set_aside(&mut search, Duration::ZERO);
    }
    assert_eq!(search.moving_due(), Some(300 * second));
}
