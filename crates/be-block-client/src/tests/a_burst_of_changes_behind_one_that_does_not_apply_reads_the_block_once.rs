use std::{
    collections::HashMap,
    sync::{
        Mutex,
        atomic::{AtomicUsize, Ordering},
    },
};

use be_block_protocol::{
    BlockSummary, ChangedReferences, MovedReferences, ReferenceChange, references_digest,
};

use super::*;
use crate::connection::{Held, whole};

/// Summaries as a device keeps them: a newer one replaces an older one, never the reverse.
#[derive(Default)]
struct Kept(Mutex<HashMap<Uuid, BlockSummary>>);

impl Held for Kept {
    fn references(&self, block: Uuid) -> Option<(u64, Vec<Uuid>)> {
        let kept = self.0.lock().unwrap();
        kept.get(&block)
            .map(|summary| (summary.revision, summary.references.clone()))
    }

    fn keep(&self, summary: &BlockSummary) {
        let mut kept = self.0.lock().unwrap();
        if kept
            .get(&summary.id)
            .is_none_or(|known| known.revision <= summary.revision)
        {
            kept.insert(summary.id, summary.clone());
        }
    }
}

fn folder(block: Uuid, revision: u64, references: Vec<Uuid>) -> BlockSummary {
    BlockSummary {
        revision,
        references,
        ..BlockSummary::no_access(block, Uuid::nil(), be_block_graph::Access::View)
    }
}

/// A burst of changes queued behind one that does not apply to what the device holds costs one
/// read of the block, not one per change: the read holds the rest of the burst, so they are
/// no news, and those it already holds apply to it, as does the next.
#[tokio::test]
async fn a_burst_of_changes_behind_one_that_does_not_apply_reads_the_block_once() {
    let block = Uuid::new_v4();
    let entries: Vec<Uuid> = (0..5).map(|_| Uuid::new_v4()).collect();
    let kept = Kept::default();
    // What the device holds is not what the server's changes start from.
    kept.keep(&folder(block, 1, vec![Uuid::new_v4()]));
    let reads = AtomicUsize::new(0);
    // The server's references after each change, at revisions 2 to 6.
    let states: Vec<Vec<Uuid>> = (1..=5).map(|count| entries[..count].to_vec()).collect();
    let announced = |change: usize| {
        let before: &[Uuid] = match change {
            0 => &[],
            _ => &states[change - 1],
        };
        let moved = ChangedReferences::Moved(MovedReferences {
            change: ReferenceChange::between(before, &states[change]),
            digest: references_digest(&states[change]),
        });
        (folder(block, 2 + change as u64, Vec::new()), moved)
    };
    // The block read again after the first four changes were made, before they arrive.
    let read = folder(block, 6, states[3].clone());

    let mut heard = Vec::new();
    for change in 0..5 {
        let (summary, references) = announced(change);
        let reread = async |_| {
            reads.fetch_add(1, Ordering::Relaxed);
            Some(read.clone())
        };
        if let Some(summary) = whole(Some(&kept), summary, references, reread).await {
            heard.push(summary.references);
        }
    }

    assert_eq!(
        reads.load(Ordering::Relaxed),
        1,
        "the block was read again more than once"
    );
    // The read, then the change it was read after, which applies to it, then the next.
    assert_eq!(
        heard,
        vec![states[3].clone(), states[3].clone(), states[4].clone()]
    );
}
