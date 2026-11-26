//! What a replica's module may reach: the parts it reads (`read-part`) and the parts it hands
//! back unchanged to be written again (`publish`, `stash`). A module is sandboxed code, perhaps
//! another app's, which serves every block of its type; reaching a part by its ref alone would
//! let it read a block it was never given, or write that block's bytes into its own for the
//! readers of its own (guides/blocks/keys.md, Keys are bound to blocks where the server
//! points).

use std::collections::{HashMap, HashSet};
use std::sync::PoisonError;

use std::sync::Mutex;

use be_block_client::{ClientError, Peer};
use be_block_store::{ObjectStore, PartRef, Written};
use uuid::Uuid;

/// What a module that names a part outside its replica's reach is told.
pub const OUTSIDE_REACH: &str =
    "a replica reaches only the parts of its block and of the content it was handed";

/// One replica's reach: the parts of the content it loaded and the parts it put (`handed`),
/// and any part sealed under a key of its block or of a block what it was handed is sealed
/// under (a checkout copy's repository, say), as a part a peer put and names in an edit is.
/// A part named by its ref alone, sealed under another block's key, is outside it.
#[derive(Debug, Default)]
pub struct Reach {
    handed: HashSet<PartRef>,
    /// Handed parts whose key's block is not yet among `sources`.
    unsourced: Vec<PartRef>,
    /// The blocks of the keys handed parts are sealed under.
    sources: HashSet<Uuid>,
    /// The parts its module's part writers finished, each pinned from the vault's collection
    /// (`Peer::collect_unnamed`) until a commit of the block that names it is sealed here (a
    /// publish of the replica's, or a merge, `named`) or the replica goes: until then only the
    /// module's edits name it.
    written: HashMap<PartRef, Vec<Written>>,
}

impl Reach {
    pub fn hand(&mut self, parts: impl IntoIterator<Item = PartRef>) {
        for part in parts {
            if self.handed.insert(part) {
                self.unsourced.push(part);
            }
        }
    }

    /// Hands `part`, which the module's part writer finished as `written`, keeping it pinned.
    pub fn hand_written(&mut self, part: PartRef, written: Written) {
        self.hand([part]);
        self.written.entry(part).or_default().push(written);
    }

    /// A commit of the block sealed here (a publish or a merge) names `parts`: what pinned them
    /// can go.
    pub fn named(&mut self, parts: impl IntoIterator<Item = PartRef>) {
        if self.written.is_empty() {
            return;
        }
        for part in parts {
            self.written.remove(&part);
        }
    }

    /// How many of the parts its module wrote are still pinned (`hand_written`).
    #[cfg(test)]
    pub fn pinned(&self) -> usize {
        self.written.len()
    }
}

/// The parts `parts` reuse by their refs: what a commit written of them names.
pub fn reused(parts: &[be_sdk_type_api::WrittenPart]) -> impl Iterator<Item = PartRef> + '_ {
    parts.iter().filter_map(|part| match part {
        be_sdk_type_api::WrittenPart::Reused(part) => Some(PartRef::from(*part)),
        _ => None,
    })
}

/// Whether `part` is within the reach of `block`'s replica.
pub async fn reaches<S: ObjectStore>(
    reach: &Mutex<Reach>,
    peer: &Peer<S>,
    block: Uuid,
    part: PartRef,
) -> Result<bool, ClientError> {
    if reach
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .handed
        .contains(&part)
    {
        return Ok(true);
    }
    let Some(of) = peer.block_of_part(part).await? else {
        return Ok(false);
    };
    if of == block
        || reach
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .sources
            .contains(&of)
    {
        return Ok(true);
    }
    // Only now: the blocks of what it was handed, which a part never reached before needs.
    // They stay listed until resolved, so a check that runs meanwhile (a replica's reads run
    // at once) resolves them too rather than find none.
    let unsourced = reach
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .unsourced
        .clone();
    let mut resolved = HashSet::new();
    let mut sources = HashSet::new();
    let mut failed = None;
    for handed in unsourced {
        match peer.block_of_part(handed).await {
            Ok(source) => {
                sources.extend(source);
                resolved.insert(handed);
            }
            Err(error) => {
                failed = Some(error);
                break;
            }
        }
    }
    let mut reach = reach.lock().unwrap_or_else(PoisonError::into_inner);
    reach.sources.extend(sources);
    reach.unsourced.retain(|part| !resolved.contains(part));
    match reach.sources.contains(&of) {
        true => Ok(true),
        false => failed.map_or(Ok(false), Err),
    }
}

/// Whether every part `parts` reuses is within the reach of `block`'s replica.
pub async fn reaches_all<S: ObjectStore>(
    reach: &Mutex<Reach>,
    peer: &Peer<S>,
    block: Uuid,
    parts: &[be_sdk_type_api::WrittenPart],
) -> Result<bool, ClientError> {
    for part in parts {
        if let be_sdk_type_api::WrittenPart::Reused(part) = part
            && !reaches(reach, peer, block, (*part).into()).await?
        {
            return Ok(false);
        }
    }
    Ok(true)
}
