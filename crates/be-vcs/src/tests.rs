use be_block::BlockMetadata;
use be_store::{ChunkerConfig, ContentKey, Keyring, Manifest, MemoryStore, Vault};
use uuid::Uuid;

use super::*;

mod a_block_edited_on_one_side_and_removed_on_the_other_is_kept_as_a_conflict;
mod a_block_left_without_its_parent_is_moved_under_the_root;
mod a_conflict_keeps_every_side_and_is_resolved_by_taking_one;
mod a_diff_names_what_was_added_removed_and_changed;
mod a_recorded_conflict_stays_until_a_side_resolves_it;
mod a_tree_reads_back_what_was_written;
mod both_sides_naming_a_block_is_a_name_conflict_and_derived_names_are_not;
mod content_both_sides_changed_is_left_for_its_content_type;
mod edits_to_different_blocks_merge_cleanly;

const CONTENT: Uuid = Uuid::from_u128(0x7465_7874);
const ROOT: Uuid = Uuid::from_u128(1);
const KEY: Uuid = Uuid::from_u128(0x6b65);

fn vault() -> Vault<MemoryStore> {
    Vault::new(MemoryStore::new(), {
        let keys = Keyring::new();
        keys.insert(KEY, ContentKey::from_bytes([3; 32]));
        keys
    })
    .with_chunker(ChunkerConfig::SMALL)
}

fn manifest(vault: &Vault<MemoryStore>, text: &str) -> Manifest {
    vault.write(KEY, CONTENT, text.as_bytes()).unwrap()
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
        ..Tree::default()
    }
}

fn kinds(tree: &Tree) -> Vec<(Uuid, ConflictKind)> {
    tree.conflicts
        .iter()
        .map(|(block, conflict)| (*block, conflict.kind))
        .collect()
}
