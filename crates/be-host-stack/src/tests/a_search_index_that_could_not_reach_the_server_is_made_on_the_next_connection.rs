use super::*;
use be_block_client::ClientError;

use crate::search::not_there_yet;

#[test]
fn a_search_index_that_could_not_reach_the_server_is_made_on_the_next_connection() {
    // Out of reach is no lasting failure: no error is shown.
    assert!(not_there_yet(&ClientError::Disconnected("offline".into())));
    assert!(not_there_yet(&ClientError::TimedOut("reading keys")));
    // A lock the keys answered is.
    assert!(!not_there_yet(&ClientError::Locked(Uuid::new_v4())));

    // The root is left alone at the revision it failed at, and tried again on the next
    // connection without waiting for the graph to move.
    let (root, revision) = (Uuid::new_v4(), 7);
    let mut search = Searching::default();
    search.make_failed(root, revision);
    assert!(search.excluded(root, revision, be_block_graph::Access::Edit));
    search.reconnected();
    assert!(!search.excluded(root, revision, be_block_graph::Access::Edit));
}
