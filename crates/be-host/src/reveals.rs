//! Places blocks were opened at (a search hit's, `editor-event.reveal`), held until the editor
//! showing the block is open.

use std::{collections::HashMap, time::Duration};

use be_sdk_api::SearchLocation;
use uuid::Uuid;

/// How long an open at a place waits for its block's editor. An open the shell never shows in an
/// editor of its own (a child elsewhere, refused) lets go of its place after this, so it cannot
/// select old words in an editor that opens later for another reason.
const PATIENCE: Duration = Duration::from_secs(5);

#[derive(Default)]
pub(crate) struct Reveals {
    waiting: HashMap<Uuid, (SearchLocation, Duration)>,
}

impl Reveals {
    /// An open of `block`, at `at` or at no place: the latest open of a block decides.
    pub(crate) fn opened(&mut self, block: Uuid, at: Option<SearchLocation>, now: Duration) {
        match at {
            Some(at) => self.waiting.insert(block, (at, now + PATIENCE)),
            None => self.waiting.remove(&block),
        };
    }

    /// The block's editor closed: whatever waited for it goes.
    pub(crate) fn closed(&mut self, block: Uuid) {
        self.waiting.remove(&block);
    }

    /// Hands each waiting place to `reveal`, which answers whether the block's editor took it;
    /// a place it took, or that waited past `PATIENCE`, goes.
    pub(crate) fn deliver(
        &mut self,
        now: Duration,
        mut reveal: impl FnMut(Uuid, &SearchLocation) -> bool,
    ) {
        self.waiting
            .retain(|block, (at, until)| now < *until && !reveal(*block, at));
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.waiting.is_empty()
    }
}

#[cfg(test)]
mod tests;
