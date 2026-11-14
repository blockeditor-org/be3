use be_block_store::Hash;

use super::*;

mod a_drain_naming_no_kept_commit_forgets_nothing;
mod edits_restored_after_a_restart_stay_sending_until_the_outbox_tip_drains;

fn commit(seed: u8) -> CommitId {
    CommitId::from_hash(Hash::from_bytes([seed; 32]))
}

fn sending(lacking: &Lacking) -> usize {
    lacking.entries.len()
}
