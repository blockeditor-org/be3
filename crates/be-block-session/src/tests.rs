use be_block_commit::{Commit, CommitId, CommitStore};
use be_block_store::{ChunkerConfig, ContentKey, Keyring, MemoryStore, Vault};
use uuid::Uuid;

use super::*;

mod replicas_fed_one_order_agree_on_the_holder_at_every_number;
mod the_holder_is_the_last_claimant_with_edit_access_else_the_first_editor;
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
            keys.insert(KEY, ContentKey::from_bytes([3; 32]), uuid::Uuid::nil());
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

fn joined(may_edit: bool, tag: be_block_store::Hash) -> be_block_protocol::MembershipChange {
    be_block_protocol::MembershipChange::Joined(be_block_protocol::JoinedParticipant {
        tag,
        may_edit,
    })
}
