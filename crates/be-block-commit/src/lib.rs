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

/// The furthest a measurement of the server's clock may pull this device's commit times back
/// (`trusts_measurement`): a server whose clock reads earlier than that is not believed, so a
/// malicious one cannot make every later edit look older than what it follows.
pub const MAX_CLOCK_PULLBACK: i64 = retention::DAY;

/// Whether a measured server clock, `server_now` against this device's `local`, is taken: one
/// that would pull commit times back by more than `MAX_CLOCK_PULLBACK` is not.
pub fn trusts_measurement(local: i64, server_now: i64) -> bool {
    server_now >= local.saturating_sub(MAX_CLOCK_PULLBACK)
}

/// The time a device stamps a commit with, `local` being its own clock: that clock, unless it
/// runs ahead of the server's estimated clock (`server_now`) by more than
/// `COMMIT_CLOCK_SLACK`, when it is the server's plus that slack, so an honest device whose
/// clock is fast is never refused; with no estimate, its own clock. Never earlier than
/// `parent`, the time of the commit it follows, as far as its own clock reaches.
pub fn commit_time(local: i64, server_now: Option<i64>, parent: Option<i64>) -> i64 {
    let stamped = match server_now {
        Some(server) => local.min(server.saturating_add(COMMIT_CLOCK_SLACK)),
        None => local,
    };
    match parent {
        Some(parent) => stamped.max(parent.min(local)),
        None => stamped,
    }
}

/// The latest commit time a device believes (`be_lib_search::Commit::believed`): the later of
/// its own clock and the server's estimated clock, plus `MAX_COMMIT_AHEAD`, so a device whose
/// clock runs slow still believes what others stamped honestly. The server's counts no more
/// than `MAX_CLOCK_PULLBACK` past its own, as a measurement is trusted no further back.
pub fn latest_believed(local: i64, server_now: Option<i64>) -> i64 {
    // A server whose clock reads far ahead would otherwise switch the bound off.
    let server = server_now
        .unwrap_or(local)
        .min(local.saturating_add(MAX_CLOCK_PULLBACK));
    local.max(server).saturating_add(MAX_COMMIT_AHEAD)
}

/// Whether `now_milliseconds` is pinned (`pin_now`): the test's clock, which an embedded
/// server in the same process reads too.
pub fn is_pinned() -> bool {
    PINNED_NOW.load(Ordering::Relaxed) != UNPINNED
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
