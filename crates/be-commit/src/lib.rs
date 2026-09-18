use std::time::{SystemTime, UNIX_EPOCH};

pub mod commit;
pub mod merge;
pub mod retention;

pub use commit::{Commit, CommitId, CommitKind, CommitStore, reference_delta};
pub use merge::{
    Conflict, MapMerge, Merge, MergeOutcome, MergeResult, merge_lines, merge_map, merge_slices,
    render_conflicts,
};
pub use retention::{CommitSummary, RetentionPlan, RetentionPolicy, plan as plan_retention};

pub fn now_milliseconds() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| {
            i64::try_from(elapsed.as_millis()).unwrap_or(i64::MAX)
        })
}

#[cfg(test)]
mod tests;
