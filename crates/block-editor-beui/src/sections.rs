use block_ui::BlockTypes;
use uuid::Uuid;

use crate::{BlockInfo, BlockParent};

pub fn is_workspace(block: &BlockInfo) -> bool {
    block.parent == Some(BlockParent::Root) && !is_system(block.block_type)
}

pub fn is_system(block_type: Uuid) -> bool {
    !be_block::is_workspace_type(block_type)
}

fn is_others_files(block: &BlockInfo, organization: Uuid, account: Uuid) -> bool {
    block.author.is_some_and(|author| {
        author != account && block.id == be_block::my_files(organization, author)
    })
}

pub fn workspaces(
    roots: Vec<BlockInfo>,
    organization: Uuid,
    account: Uuid,
    types: &dyn BlockTypes,
) -> Vec<BlockInfo> {
    let mine = be_block::my_files(organization, account);
    let mut found: Vec<(bool, String, BlockInfo)> = roots
        .into_iter()
        .filter(is_workspace)
        .filter(|block| !is_others_files(block, organization, account))
        .map(|block| {
            let name = block.label(types).name.to_lowercase();
            (block.id != mine, name, block)
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
        let kind = be_block::SYSTEM_TYPES
            .iter()
            .position(|kind| *kind == block.block_type);
        (kind, block.id)
    });
    found
}
