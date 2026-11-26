use super::*;

use be_block_store::ObjectStore;

/// The server thins a block's early history, so the remote head's walk can stop short of the
/// head a replica is on: that is no divergence without a base, which would hold both sides'
/// text twice, but a merge on the head the replica is on.
#[test]
fn resuming_past_thinned_history_merges_on_the_head_it_was_on() {
    let commits = commits();
    let local = write(&commits, "shared\n", 1_000, None);
    let thinned = write(&commits, "shared\nmore\n", 2_000, Some(local));
    let remote = write(&commits, "shared\nmore\nremote\n", 3_000, Some(thinned));
    commits.vault().store().remove(thinned.hash()).unwrap();

    assert_eq!(
        resume(&commits, Some(local), Some(remote)).unwrap(),
        Resume::Merge {
            base: Some(local),
            ours: local,
            theirs: remote,
        }
    );
}
