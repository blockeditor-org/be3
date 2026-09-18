use be_commit::{Commit, CommitId, CommitStore};
use be_store::{ChunkerConfig, ContentKey, MemoryStore, Vault};
use uuid::Uuid;

use super::*;

mod a_clean_handoff_needs_no_merge;
mod a_follower_resubmits_only_what_the_owner_never_accepted;
mod an_expired_lease_can_be_claimed_but_a_held_one_cannot;
mod ownership_moves_on_when_the_owner_leaves;
mod resuming_only_merges_when_history_actually_diverged;

const CONTENT: Uuid = Uuid::from_u128(0x7465_7874);
const AUTHOR: Uuid = Uuid::from_u128(0xa170);

fn commits() -> CommitStore<MemoryStore> {
    CommitStore::new(
        Vault::new(MemoryStore::new(), ContentKey::from_bytes([3; 32]))
            .with_chunker(ChunkerConfig::SMALL),
    )
}

fn write(
    commits: &CommitStore<MemoryStore>,
    text: &str,
    time: i64,
    parent: Option<CommitId>,
) -> CommitId {
    commits
        .write(CONTENT, text.as_bytes(), AUTHOR, time, parent, Vec::new())
        .unwrap()
}

fn merged(
    commits: &CommitStore<MemoryStore>,
    text: &str,
    time: i64,
    parents: Vec<CommitId>,
) -> CommitId {
    let manifest = commits.vault().write(CONTENT, text.as_bytes()).unwrap();
    commits
        .put(&Commit::new(manifest, AUTHOR, time).with_parents(parents))
        .unwrap()
}
