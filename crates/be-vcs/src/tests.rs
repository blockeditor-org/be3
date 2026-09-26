use be_block::BlockMetadata;
use be_store::{ChunkerConfig, ContentKey, Manifest, MemoryStore, Vault};
use uuid::Uuid;

use super::*;

mod a_block_edited_on_one_side_and_removed_on_the_other_is_kept_as_a_conflict;
mod a_block_left_without_its_parent_is_moved_under_the_root;
mod a_diff_names_what_was_added_removed_and_changed;
mod a_tree_reads_back_what_was_written;
mod content_both_sides_changed_is_left_for_its_content_type;
mod edits_to_different_blocks_merge_cleanly;

const CONTENT: Uuid = Uuid::from_u128(0x7465_7874);
const ROOT: Uuid = Uuid::from_u128(1);

fn vault() -> Vault<MemoryStore> {
    Vault::new(MemoryStore::new(), ContentKey::from_bytes([3; 32]))
        .with_chunker(ChunkerConfig::SMALL)
}

fn manifest(vault: &Vault<MemoryStore>, text: &str) -> Manifest {
    vault.write(CONTENT, text.as_bytes()).unwrap()
}

fn entry(parent: Option<Uuid>, content: Manifest) -> Entry {
    Entry {
        content_type: CONTENT,
        parent,
        metadata: BlockMetadata::default(),
        content: Some(content),
        references: Vec::new(),
    }
}

fn tree(entries: impl IntoIterator<Item = (Uuid, Entry)>) -> Tree {
    Tree {
        root: Some(ROOT),
        entries: entries.into_iter().collect(),
    }
}
