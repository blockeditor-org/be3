use be_block::{BlockContent, MY_FILES, SettingsContent, SettingsProfileContent};
use block_ui::BlockTypes;
use uuid::Uuid;

use crate::{BlockInfo, BlockParent};

const SYSTEM_TYPES: [Uuid; 2] = [
    SettingsContent::CONTENT_TYPE,
    SettingsProfileContent::CONTENT_TYPE,
];

pub fn is_workspace(block: &BlockInfo) -> bool {
    block.parent == Some(BlockParent::Root) && !is_system(block.block_type)
}

pub fn is_system(block_type: Uuid) -> bool {
    SYSTEM_TYPES.contains(&block_type)
}

pub fn is_my_files(block: &BlockInfo, account: Uuid) -> bool {
    is_workspace(block)
        && block.author == Some(account)
        && block.metadata.get(&MY_FILES) == Some(true)
}

pub fn workspaces(roots: Vec<BlockInfo>, account: Uuid, types: &dyn BlockTypes) -> Vec<BlockInfo> {
    let mut found: Vec<(bool, String, BlockInfo)> = roots
        .into_iter()
        .filter(is_workspace)
        .filter(|block| {
            block.metadata.get(&MY_FILES) != Some(true) || block.author == Some(account)
        })
        .map(|block| {
            let name = block.label(types).name.to_lowercase();
            (!is_my_files(&block, account), name, block)
        })
        .collect();
    found.sort_by(|left, right| (left.0, &left.1, left.2.id).cmp(&(right.0, &right.1, right.2.id)));
    found.into_iter().map(|(_, _, block)| block).collect()
}

pub fn shared_with_me(shared: Vec<BlockInfo>) -> Vec<BlockInfo> {
    shared
        .into_iter()
        .filter(|block| !is_system(block.block_type))
        .collect()
}

pub fn own_system(roots: Vec<BlockInfo>, account: Uuid) -> Vec<BlockInfo> {
    let mut found: Vec<BlockInfo> = roots
        .into_iter()
        .filter(|block| is_system(block.block_type) && block.author == Some(account))
        .collect();
    found.sort_by_key(|block| {
        let kind = SYSTEM_TYPES
            .iter()
            .position(|kind| *kind == block.block_type);
        (kind, block.id)
    });
    found
}
