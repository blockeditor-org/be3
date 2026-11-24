use be_lib_stable_hash::HashMap;

use uuid::Uuid;

use super::*;
use crate::BlockParent;

mod a_block_on_its_own_path_is_shown_but_not_unfolded;
mod a_folder_with_nothing_shown_gets_a_note_in_place_of_its_children;
mod a_search_keeps_what_matches_and_the_blocks_above_it;

/// A block `n` in `parent`, referencing `children`.
fn block(n: u128, parent: u128, children: &[u128]) -> BlockInfo {
    let mut block = BlockInfo::new(
        Uuid::from_u128(n),
        Uuid::nil(),
        BlockParent::Block(Uuid::from_u128(parent)),
    );
    block.references = children.iter().copied().map(Uuid::from_u128).collect();
    block
}

/// A graph of `blocks`, each listing the blocks it references.
fn graph(blocks: &[BlockInfo]) -> HashMap<Uuid, BlockInfo> {
    blocks.iter().map(|block| (block.id, block.clone())).collect()
}

fn children_of(graph: &HashMap<Uuid, BlockInfo>, block: &BlockInfo) -> Option<Listed> {
    Some(Listed::Blocks(
        block
            .references
            .iter()
            .filter_map(|id| graph.get(id).cloned())
            .collect(),
    ))
}

/// Each row as its path's numbers, a note as its path then 0.
fn paths(rows: &[TreeEntry]) -> Vec<Vec<u128>> {
    rows.iter()
        .map(|row| match row {
            TreeEntry::Block(row) => row.path.iter().map(|id| id.as_u128()).collect(),
            TreeEntry::Note { path, .. } => path
                .iter()
                .map(|id| id.as_u128())
                .chain([0])
                .collect(),
        })
        .collect()
}
