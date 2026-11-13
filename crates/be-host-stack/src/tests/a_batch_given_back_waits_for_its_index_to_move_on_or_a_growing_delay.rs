use super::*;

use std::time::Duration;

use be_block_commit::CommitId;

use crate::search::Searching;

fn commit(n: u8) -> CommitId {
    CommitId::from_hash(be_block_store::Hash::from_bytes([n; 32]))
}

#[test]
fn a_batch_given_back_waits_for_its_index_to_move_on_or_a_growing_delay() {
    let (index, block) = (Uuid::new_v4(), Uuid::new_v4());
    let given_back = (Some(commit(1)), Some(commit(1)));
    let second = Duration::from_secs(1);
    let mut search = Searching::default();

    // Held back, not tried again at once, while the index stays where it was given back.
    search.hold_moving(index, vec![block], given_back);
    search.release_moving(Duration::ZERO, |_| given_back, |_| true);
    assert!(search.take_queued().is_empty());
    assert!(!search.indexed());
    assert_eq!(search.moving_due(), Some(second));

    // Its next head, which another device's publish moves it to, gives the next try.
    search.release_moving(
        Duration::ZERO,
        |_| (Some(commit(2)), Some(commit(1))),
        |_| true,
    );
    assert_eq!(search.take_queued(), [block]);
    assert_eq!(search.moving_due(), None);

    // Given back again in a row, it waits twice as long, and goes once that is up.
    search.hold_moving(index, vec![block], given_back);
    search.release_moving(Duration::ZERO, |_| given_back, |_| true);
    assert_eq!(search.moving_due(), Some(2 * second));
    search.release_moving(second, |_| given_back, |_| true);
    assert!(search.take_queued().is_empty());
    search.release_moving(2 * second, |_| given_back, |_| true);
    assert_eq!(search.take_queued(), [block]);

    // An index no root names any more lets its batch go, to be found no index.
    search.hold_moving(index, vec![block], given_back);
    search.release_moving(Duration::ZERO, |_| given_back, |_| false);
    assert_eq!(search.take_queued(), [block]);

    // A batch written starts the delay over.
    search.wrote(index);
    search.hold_moving(index, vec![block], given_back);
    search.release_moving(Duration::ZERO, |_| given_back, |_| true);
    assert_eq!(search.moving_due(), Some(second));
}
