use be_store::{ChunkerConfig, ContentKey, MemoryStore, Vault};
use uuid::Uuid;

use super::*;
use crate::{commit::Commit, merge::merge_lines, retention::*};

mod a_commit_chain_walks_to_its_root;
mod a_common_ancestor_is_found_across_a_fork;
mod a_cut_and_paste_conflicts_instead_of_interleaving;
mod a_keyed_map_merges_per_entry;
mod identical_edits_on_both_sides_merge_cleanly;
mod pruned_history_leaves_the_remaining_chain_walkable;
mod retention_keeps_bookmarks_and_recent_states;
mod retention_keeps_what_it_kept_as_time_passes;
mod retention_prefers_states_that_were_left_alone;
mod three_way_merge_combines_edits_to_separate_regions;
mod three_way_merge_conflicts_when_both_sides_rewrite_a_region;

const CONTENT: Uuid = Uuid::from_u128(0x7465_7874_2d63_6f6e_7465_6e74_2d30_3031);
const AUTHOR: Uuid = Uuid::from_u128(0xa170);

fn store() -> CommitStore<MemoryStore> {
    CommitStore::new(
        Vault::new(MemoryStore::new(), ContentKey::from_bytes([9; 32]))
            .with_chunker(ChunkerConfig::SMALL),
    )
}

fn commit(
    store: &CommitStore<MemoryStore>,
    text: &str,
    time: i64,
    parent: Option<CommitId>,
) -> CommitId {
    store
        .write(CONTENT, text.as_bytes(), AUTHOR, time, parent, Vec::new())
        .unwrap()
}

fn text(outcome: &MergeOutcome<Vec<u8>>) -> String {
    String::from_utf8(merge::join_lines(&outcome.merged)).unwrap()
}

fn summary(id: CommitId, time: i64) -> CommitSummary {
    CommitSummary {
        id,
        time,
        pinned: false,
    }
}
