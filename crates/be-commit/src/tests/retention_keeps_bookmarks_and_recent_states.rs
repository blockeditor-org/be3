use super::*;

#[test]
fn retention_keeps_bookmarks_and_recent_states() {
    let now = 365 * DAY;
    let policy = RetentionPolicy::default();
    let bookmark = CommitSummary {
        id: CommitId::from_hash(be_store::Hash::of(b"bookmark")),
        time: now - 200 * DAY,
        pinned: true,
    };
    let mut summaries = vec![bookmark];
    for step in 0..400i64 {
        summaries.push(summary(
            CommitId::from_hash(be_store::Hash::of(&step.to_le_bytes())),
            now - step * 30 * MINUTE,
        ));
    }

    let plan = plan(&summaries, now, &policy);
    assert!(plan.keep.contains(&bookmark.id));
    for summary in &summaries {
        if now - summary.time <= policy.keep_all_within {
            assert!(plan.keep.contains(&summary.id));
        }
    }
    assert!(
        plan.keep.len() < summaries.len() / 2,
        "thinning kept {} of {} states",
        plan.keep.len(),
        summaries.len()
    );
    assert_eq!(plan.keep.len() + plan.drop.len(), summaries.len());
}
