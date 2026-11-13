//! Places blocks were opened at (a search hit's, `editor-event.reveal`), held until the editor
//! showing the block is open.

use std::{collections::HashMap, time::Duration};

use be_sdk_api::SearchLocation;
use uuid::Uuid;

/// How long an open at a place waits for its block's editor to exist. An open the shell never
/// shows in an editor of its own (a child elsewhere, refused) lets go of its place after this, so
/// it cannot select old words in an editor that opens later for another reason. The clock runs
/// only while there is no editor: one that exists but is still opening gets the place however
/// long it takes, until it is told or closed.
const PATIENCE: Duration = Duration::from_secs(5);

/// What became of a place handed to its block's editor.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Delivery {
    /// The editor took the place.
    Told,
    /// The block has an editor, still opening: the place waits for it.
    Opening,
    /// The block has no editor: the place waits until `PATIENCE` runs out.
    NoEditor,
}

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

    /// Hands each waiting place to `reveal`. A place its editor took goes; one whose editor is
    /// still opening waits on, its clock restarted; one with no editor goes once `PATIENCE` has
    /// passed without one.
    pub(crate) fn deliver(
        &mut self,
        now: Duration,
        mut reveal: impl FnMut(Uuid, &SearchLocation) -> Delivery,
    ) {
        self.waiting
            .retain(|block, (at, until)| match reveal(*block, at) {
                Delivery::Told => false,
                Delivery::Opening => {
                    *until = now + PATIENCE;
                    true
                }
                Delivery::NoEditor => now < *until,
            });
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.waiting.is_empty()
    }
}

#[cfg(test)]
mod tests;
