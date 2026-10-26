use std::time::{SystemTime, UNIX_EPOCH};

pub mod commit;
pub mod merge;
pub mod retention;
#[cfg(feature = "store")]
mod store;

pub use commit::{ChangeId, Commit, CommitId, CommitKind};
pub use merge::{
    Conflict, MapMerge, Merge, MergeOutcome, MergeResult, merge_lines, merge_map, merge_slices,
    render_conflicts,
};
pub use retention::{CommitSummary, RetentionPlan, RetentionPolicy, plan as plan_retention};
#[cfg(feature = "store")]
pub use store::CommitStore;

pub fn now_milliseconds() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| {
            i64::try_from(elapsed.as_millis()).unwrap_or(i64::MAX)
        })
}

#[cfg(test)]
mod tests;
