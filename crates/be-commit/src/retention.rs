use std::collections::HashMap;

use crate::commit::CommitId;

pub const MINUTE: i64 = 60 * 1000;
pub const HOUR: i64 = 60 * MINUTE;
pub const DAY: i64 = 24 * HOUR;
pub const WEEK: i64 = 7 * DAY;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Bucket {
    pub until_age: i64,
    pub interval: i64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RetentionPolicy {
    pub keep_all_within: i64,
    pub buckets: Vec<Bucket>,
}

impl RetentionPolicy {
    pub fn interval_for(&self, age: i64) -> i64 {
        self.buckets
            .iter()
            .find(|bucket| age <= bucket.until_age)
            .map_or(WEEK, |bucket| bucket.interval)
    }

    fn bucket_of(&self, age: i64) -> usize {
        self.buckets
            .iter()
            .position(|bucket| age <= bucket.until_age)
            .unwrap_or(self.buckets.len())
    }
}

impl Default for RetentionPolicy {
    fn default() -> Self {
        Self {
            keep_all_within: HOUR,
            buckets: vec![
                Bucket {
                    until_age: DAY,
                    interval: 10 * MINUTE,
                },
                Bucket {
                    until_age: WEEK,
                    interval: HOUR,
                },
                Bucket {
                    until_age: 30 * DAY,
                    interval: DAY,
                },
                Bucket {
                    until_age: i64::MAX,
                    interval: WEEK,
                },
            ],
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CommitSummary {
    pub id: CommitId,
    pub time: i64,
    pub pinned: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RetentionPlan {
    pub keep: Vec<CommitId>,
    pub drop: Vec<CommitId>,
}

pub fn plan(summaries: &[CommitSummary], now: i64, policy: &RetentionPolicy) -> RetentionPlan {
    let mut ordered: Vec<_> = summaries.to_vec();
    ordered.sort_by(|left, right| right.time.cmp(&left.time).then(right.id.cmp(&left.id)));

    let mut candidates: HashMap<(usize, i64), usize> = HashMap::new();
    let mut plan = RetentionPlan::default();
    let mut undecided = Vec::new();

    for (index, summary) in ordered.iter().enumerate() {
        let age = now - summary.time;
        if index == 0 || summary.pinned || age <= policy.keep_all_within {
            plan.keep.push(summary.id);
            continue;
        }
        let bucket = policy.bucket_of(age);
        let interval = policy.interval_for(age).max(1);
        let slot = age / interval;
        let quiet = ordered[index - 1].time - summary.time;
        let position = undecided.len();
        undecided.push((summary.id, quiet, summary.time));
        candidates
            .entry((bucket, slot))
            .and_modify(|best| {
                let (_, best_quiet, best_time) = undecided[*best];
                if quiet > best_quiet || (quiet == best_quiet && summary.time > best_time) {
                    *best = position;
                }
            })
            .or_insert(position);
    }

    let mut kept: Vec<_> = candidates.values().copied().collect();
    kept.sort_unstable();
    let mut kept = kept.into_iter().peekable();
    for (position, (id, _, _)) in undecided.iter().enumerate() {
        if kept.peek() == Some(&position) {
            kept.next();
            plan.keep.push(*id);
        } else {
            plan.drop.push(*id);
        }
    }
    plan
}
