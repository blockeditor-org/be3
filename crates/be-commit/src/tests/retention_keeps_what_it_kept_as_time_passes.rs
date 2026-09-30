use super::*;

#[test]
fn retention_keeps_what_it_kept_as_time_passes() {
    let now = 365 * DAY;
    let policy = RetentionPolicy::default();
    let mut summaries = vec![summary(
        CommitId::from_hash(be_store::Hash::of(b"head")),
        now,
    )];
    for step in 0..150i64 {
        summaries.push(summary(
            CommitId::from_hash(be_store::Hash::of(&step.to_le_bytes())),
            now - 2 * DAY - step * 40 * MINUTE,
        ));
    }

    let first = plan(&summaries, now, &policy);
    let survivors: Vec<_> = summaries
        .iter()
        .copied()
        .filter(|summary| first.keep.contains(&summary.id))
        .collect();
    for later in [10 * MINUTE, 25 * MINUTE, 50 * MINUTE] {
        let second = plan(&survivors, now + later, &policy);
        assert!(
            second.drop.is_empty(),
            "{} minutes later, {} kept states were dropped",
            later / MINUTE,
            second.drop.len()
        );
    }
}
