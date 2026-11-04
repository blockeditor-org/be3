//! Cut records: a cut of block references is recorded here, not in the clip,
//! so only the host can say a paste moves a block. The first paste of a block
//! uses its record up; later pastes add references.

use std::collections::BTreeMap;
use std::sync::{Mutex, PoisonError};

use uuid::Uuid;

/// Where a block was cut from, and whether that place was its parent.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Cut {
    pub(crate) holder: Uuid,
    pub(crate) was_parent: bool,
}

static CUTS: Mutex<BTreeMap<Uuid, Cut>> = Mutex::new(BTreeMap::new());

/// Records a cut of `blocks` from `holder`, replacing the cuts before it.
pub(crate) fn record(blocks: impl IntoIterator<Item = (Uuid, Cut)>) {
    *CUTS.lock().unwrap_or_else(PoisonError::into_inner) = blocks.into_iter().collect();
}

/// Uses up the record of `block`'s cut, if it has one.
pub(crate) fn take(block: Uuid) -> Option<Cut> {
    CUTS.lock()
        .unwrap_or_else(PoisonError::into_inner)
        .remove(&block)
}
