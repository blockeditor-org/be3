use be_commit::{Commit, CommitId, CommitStore};
use be_store::{ChunkerConfig, ContentKey, Keyring, MemoryStore, Vault};
use uuid::Uuid;

use super::*;

mod a_clean_handoff_needs_no_merge;
mod a_follower_resubmits_only_what_the_owner_never_accepted;
mod a_new_owner_keeps_the_numbering_of_what_the_old_one_accepted_late;
mod a_sequencer_that_takes_over_owes_a_seal_for_what_it_inherited;
mod an_expired_lease_can_be_claimed_but_a_held_one_cannot;
mod ownership_moves_on_when_the_owner_leaves;
mod resuming_only_merges_when_history_actually_diverged;

const CONTENT: Uuid = Uuid::from_u128(0x7465_7874);
const AUTHOR: Uuid = Uuid::from_u128(0xa170);
const KEY: Uuid = Uuid::from_u128(0x6b65);

fn commits() -> CommitStore<MemoryStore> {
    CommitStore::new(
        Vault::new(MemoryStore::new(), {
            let keys = Keyring::new();
            keys.insert(KEY, ContentKey::from_bytes([3; 32]));
            keys
        })
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
        .write(
            KEY,
            CONTENT,
            text.as_bytes(),
            AUTHOR,
            time,
            parent,
            Vec::new(),
        )
        .unwrap()
}

fn merged(
    commits: &CommitStore<MemoryStore>,
    text: &str,
    time: i64,
    parents: Vec<CommitId>,
) -> CommitId {
    let manifest = commits
        .vault()
        .write(KEY, CONTENT, text.as_bytes())
        .unwrap();
    commits
        .put(
            KEY,
            &Commit::new(manifest, AUTHOR, time).with_parents(parents),
        )
        .unwrap()
}
