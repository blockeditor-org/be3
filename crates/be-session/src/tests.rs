use be_commit::{Commit, CommitId, CommitStore};
use be_store::{ChunkerConfig, ContentKey, Keyring, MemoryStore, Vault};
use uuid::Uuid;

use super::*;

mod a_follower_resubmits_only_what_the_server_never_sequenced;
mod a_seal_trims_the_log_but_keeps_what_later_operations_rebase_onto;
mod an_expired_lease_can_be_claimed_but_a_held_one_cannot;
mod operations_sequenced_after_a_barrier_carry_it_as_their_floor;
mod ownership_moves_on_when_the_owner_leaves;
mod resuming_only_merges_when_history_actually_diverged;
mod the_first_participant_names_the_build_that_may_speak;
mod the_sequencer_numbers_each_submission_once;

const CONTENT: Uuid = Uuid::from_u128(0x7465_7874);
const AUTHOR: Uuid = Uuid::from_u128(0xa170);
const KEY: Uuid = Uuid::from_u128(0x6b65);
const BUILD: [u8; 32] = [1; 32];

fn id(origin: u64, counter: u64) -> OpId {
    OpId { origin, counter }
}

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
