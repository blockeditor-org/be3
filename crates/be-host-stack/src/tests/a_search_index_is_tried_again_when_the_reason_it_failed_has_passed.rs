use be_block_graph::Access;
use uuid::Uuid;

use crate::search::Searching;

#[test]
fn a_search_index_is_tried_again_when_the_reason_it_failed_has_passed() {
    let (root, index) = (Uuid::new_v4(), Uuid::new_v4());
    let mut search = Searching::default();

    // A failure to make it waits for the graph to move, and does not keep the host busy.
    search.wait_for_index(root, index, Uuid::new_v4());
    search.make_failed(root, 7);
    assert!(search.excluded(root, 7, Access::Edit));
    assert!(!search.excluded(root, 8, Access::Edit), "the graph moved");

    // A refusal waits for the access to change: demoted, then promoted again, it is tried.
    search.give_up(root, Access::Edit);
    assert!(search.excluded(root, 9, Access::Edit));
    search.prune_given_up(|_| Some(Access::Edit));
    assert!(
        search.excluded(root, 9, Access::Edit),
        "the access is the same"
    );
    search.prune_given_up(|_| Some(Access::View));
    search.prune_given_up(|_| Some(Access::Edit));
    assert!(
        !search.excluded(root, 10, Access::Edit),
        "promoted again, it is no longer given up on"
    );
}
