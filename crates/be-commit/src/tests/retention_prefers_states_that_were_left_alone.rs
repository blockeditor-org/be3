use super::*;

#[test]
fn retention_prefers_states_that_were_left_alone() {
    let now = 100 * DAY;
    let policy = RetentionPolicy::default();
    let quiet = CommitId::from_hash(be_store::Hash::of(b"quiet"));
    let mut summaries = vec![
        summary(CommitId::from_hash(be_store::Hash::of(b"head")), now),
        summary(quiet, now - 10 * DAY - 6 * HOUR),
    ];
    for burst in 0..8i64 {
        summaries.push(summary(
            CommitId::from_hash(be_store::Hash::of(&burst.to_le_bytes())),
            now - 10 * DAY - 7 * HOUR + burst * MINUTE,
        ));
    }

    let plan = plan(&summaries, now, &policy);
    assert!(
        plan.keep.contains(&quiet),
        "the state left alone was dropped"
    );
    assert_eq!(plan.keep.len() + plan.drop.len(), summaries.len());
    assert!(
        plan.drop.len() >= 7,
        "an eight commit burst kept {} states",
        plan.keep.len()
    );
}
