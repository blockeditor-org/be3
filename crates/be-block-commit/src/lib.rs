use std::sync::atomic::{AtomicI64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

pub mod commit;
pub mod merge;
pub mod retention;
#[cfg(feature = "store")]
mod store;

pub use commit::{ChangeId, Commit, CommitId, CommitKind, SealedThrough};
pub use merge::{
    Conflict, Merge, MergeOutcome, MergeResult, merge_lines, merge_slices, render_conflicts,
};
pub use retention::{CommitSummary, RetentionPlan, RetentionPolicy, plan as plan_retention};
#[cfg(feature = "store")]
pub use store::CommitStore;

/// How far past the server's clock a commit's time may be: a device's clock may run a little
/// fast, but a far-future time would win every comparison by commit time, as a search query's
/// merge makes, and escape history's thinning by age. The server refuses a later one; a
/// search reader does not believe one past its own clock plus this (`be_lib_search::Commit::believed`).
pub const MAX_COMMIT_AHEAD: i64 = retention::HOUR;

/// How far past the server's clock a device stamps a commit at most, once it knows that clock
/// (`commit_time`): well inside `MAX_COMMIT_AHEAD`, which the measurement's error stays under.
pub const COMMIT_CLOCK_SLACK: i64 = retention::MINUTE;

/// The time a device stamps a commit with, `local` being its own clock: that clock, unless it
/// runs ahead of the server's by more than `COMMIT_CLOCK_SLACK`, when it is the server's clock
/// plus that slack, so an honest device whose clock is fast is never refused. `server_offset`
/// is the server's clock less this device's, as last measured; with none, its own clock.
pub fn commit_time(local: i64, server_offset: Option<i64>) -> i64 {
    match server_offset {
        Some(offset) => local.min(
            local
                .saturating_add(offset)
                .saturating_add(COMMIT_CLOCK_SLACK),
        ),
        None => local,
    }
}

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
