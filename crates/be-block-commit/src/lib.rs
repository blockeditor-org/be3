use std::sync::atomic::{AtomicI64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

pub mod commit;
pub mod merge;
pub mod retention;
#[cfg(feature = "store")]
mod store;

pub use commit::{ChangeId, Commit, CommitId, CommitKind};
pub use merge::{
    Conflict, Merge, MergeOutcome, MergeResult, merge_lines, merge_slices, render_conflicts,
};
pub use retention::{CommitSummary, RetentionPlan, RetentionPolicy, plan as plan_retention};
#[cfg(feature = "store")]
pub use store::CommitStore;

static PINNED_NOW: AtomicI64 = AtomicI64::new(UNPINNED);
const UNPINNED: i64 = i64::MIN;

/// The time commits are stamped with and the server's retention reads, in milliseconds since
/// the epoch: the system clock unless `pin_now` pinned it.
pub fn now_milliseconds() -> i64 {
    match PINNED_NOW.load(Ordering::Relaxed) {
        UNPINNED => SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |elapsed| {
                i64::try_from(elapsed.as_millis()).unwrap_or(i64::MAX)
            }),
        pinned => pinned,
    }
}

/// Pins `now_milliseconds` for the whole process, `None` unpins it. A test drives it: be-host
/// pins it to its wall clock while that is pinned, and be-host-stack's harness sets it, so
/// commit times, and the history an embedded server keeps, follow the test's clock.
pub fn pin_now(milliseconds: Option<i64>) {
    PINNED_NOW.store(
        milliseconds.map_or(UNPINNED, |at| at.max(UNPINNED + 1)),
        Ordering::Relaxed,
    );
}

#[cfg(test)]
mod tests;
