use super::*;

use be_block_store::ObjectStore;

/// The server thins a block's early history, so the remote head's walk can stop short of where
/// the replica's history meets it. That is no divergence without a base, which would hold both
/// sides' text twice, but a merge on the newest commit of the replica's history the server took
/// (`left`): the remote head was made on it.
#[test]
fn resuming_past_thinned_history_merges_on_the_head_it_was_on() {
    let commits = commits();
    let first = write(&commits, "shared\n", 1_000, None);
    let thinned = write(&commits, "shared\nmore\n", 2_000, Some(first));
    let remote = write(&commits, "shared\nmore\nremote\n", 3_000, Some(thinned));
    // Sealed here on `first` and not yet published.
    let unsent = write(&commits, "shared\nlocal\n", 2_500, Some(first));
    commits.vault().store().remove(thinned.hash()).unwrap();

    // A published head the remote one was made on.
    assert_eq!(
        resume(&commits, Some(first), Some(remote), Some(first)).unwrap(),
        Resume::Merge {
            base: Some(first),
            ours: first,
            theirs: remote,
        }
    );
    // An unpublished one merges on the head it was sealed on, keeping its own edit.
    assert_eq!(
        resume(&commits, Some(unsent), Some(remote), Some(first)).unwrap(),
        Resume::Merge {
            base: Some(first),
            ours: unsent,
            theirs: remote,
        }
    );
    // Built on no head the server took: on nothing.
    assert_eq!(
        resume(&commits, Some(unsent), Some(remote), None).unwrap(),
        Resume::Merge {
            base: None,
            ours: unsent,
            theirs: remote,
        }
    );
}
