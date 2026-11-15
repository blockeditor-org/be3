use be_block_commit::{Commit, CommitId, CommitStore};
use be_block_store::{ChunkerConfig, ContentKey, Keyring, MemoryStore, Vault};
use uuid::Uuid;

use super::*;

mod an_expired_lease_can_be_claimed_but_a_held_one_cannot;
mod only_a_participant_that_may_edit_is_granted_the_lease;
mod ownership_moves_on_when_the_owner_leaves;
mod resuming_only_merges_when_history_actually_diverged;
mod the_first_participant_names_the_build_the_session_speaks;
mod the_order_numbers_each_submission_and_carries_its_floor;

const CONTENT: Uuid = Uuid::from_u128(0x7465_7874);
const AUTHOR: Uuid = Uuid::from_u128(0xa170);
const KEY: Uuid = Uuid::from_u128(0x6b65);
const TAG: be_block_store::Hash = be_block_store::Hash::from_bytes([7; 32]);

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
