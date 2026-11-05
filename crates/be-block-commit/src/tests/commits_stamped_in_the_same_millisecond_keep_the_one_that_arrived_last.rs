use super::*;

#[test]
fn commits_stamped_in_the_same_millisecond_keep_the_one_that_arrived_last() {
    let now = 100 * DAY;
    let policy = RetentionPolicy::default();
    let at = now - 30 * MINUTE;
    let ids: Vec<CommitId> = [b"zz", b"aa", b"mm"]
        .into_iter()
        .map(|seed| CommitId::from_hash(be_block_store::Hash::of(seed)))
        .collect();
    // Listed newest first, as the server lists them: the last to arrive is the first.
    let summaries: Vec<_> = ids.iter().map(|id| summary(*id, at)).collect();

    let plan = plan(&summaries, now, &policy);
    assert_eq!(plan.keep, [ids[0]], "an earlier state outranked the last one");
    assert_eq!(plan.drop.len(), 2);
}
