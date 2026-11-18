//! Blocks that pressed notifications name, waiting until the host can open them
//! (`Shell::open_pressed_notifications`). The web build's page (`web_push`) and Android's
//! activity (`android_push`) hold them, the latter from the activity's thread, so they are kept
//! for the whole process.

use std::sync::{Mutex, PoisonError};
use std::time::{Duration, Instant};

use uuid::Uuid;

/// A block a pressed notification named, waiting until the host can open it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Opening {
    pub(crate) organization: Uuid,
    pub(crate) block: Uuid,
    /// When the notification was pressed.
    pub(crate) pressed: Instant,
}

/// How long an opening waits before it is dropped, so a press made while changes were unsaved
/// does not switch organization much later, unprompted.
pub(crate) const OPENING_PATIENCE: Duration = Duration::from_secs(30);

static OPENINGS: Mutex<Vec<Opening>> = Mutex::new(Vec::new());

pub(crate) fn has_openings() -> bool {
    !OPENINGS
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .is_empty()
}

pub(crate) fn take_openings() -> Vec<Opening> {
    std::mem::take(&mut *OPENINGS.lock().unwrap_or_else(PoisonError::into_inner))
}

/// Holds an opening that could not happen yet: the host tries again when its organization or
/// graph changes.
pub(crate) fn hold(opening: Opening) {
    OPENINGS
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .push(opening);
}

/// A notification about `block` in `organization` was pressed: the host opens it on its next
/// frame.
pub(crate) fn pressed(organization: Uuid, block: Uuid) {
    hold(Opening {
        organization,
        block,
        pressed: Instant::now(),
    });
    be_host_base::host::wake();
}
