use uuid::Uuid;

use crate::search::Searching;

#[test]
fn the_search_index_queue_forgets_what_can_no_longer_be_indexed() {
    let (root, index, block) = (Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4());

    // An index that could not be made hands its waiting blocks back, so the queue empties.
    let mut search = Searching::default();
    search.wait_for_index(root, index, block);
    assert!(!search.indexed(), "a block waits for its root's index");
    search.make_failed(root);
    assert!(
        !search.indexed(),
        "the block is queued again, to find no index"
    );
    search.take_queued();
    assert!(search.indexed(), "nothing is left waiting for the index");

    // Blocks parked for an index that has no head wait only while a root still names it.
    let mut search = Searching::default();
    search.park(index, vec![block]);
    search.release_parked(|_| false, |_| true);
    assert!(!search.indexed(), "still parked");
    search.release_parked(|_| false, |_| false);
    search.take_queued();
    assert!(
        search.indexed(),
        "no root names the index: they dropped out"
    );

    let mut search = Searching::default();
    search.park(index, vec![block]);
    search.release_parked(|_| true, |_| true);
    assert_eq!(search.take_queued(), vec![block], "its head arrived");
    assert!(search.indexed());

    // An index being made or named keeps the host busy, and so does a workspace that wants
    // one, until the root is given up on.
    let mut search = Searching::default();
    search.wait_for_index(root, index, block);
    search.take_waiting_for_test();
    assert!(search.busy(false), "an index being named is busy");
    search.make_failed(root);
    search.take_queued();
    assert!(!search.busy(false), "a root given up on is not");
    assert!(search.busy(true), "a workspace wanting its index is");
}
