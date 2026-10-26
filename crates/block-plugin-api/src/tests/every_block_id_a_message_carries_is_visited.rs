use crate::{AccessLevel, BlockIdRole, BlockInfo, BlockLocation, VersionCommand};

#[test]
fn every_block_id_a_message_carries_is_visited() {
    let mut info = BlockInfo {
        block_id: [1; 16],
        block_type: [9; 16],
        author: Some([9; 16]),
        parent: Some(BlockLocation::Block([1; 16])),
        references: vec![[1; 16]],
        access: AccessLevel::Edit,
        metadata: crate::BlockMetadata::default(),
        locked: false,
        deleted: false,
    };
    let mut command = VersionCommand::Adopt { block_id: [1; 16] };
    let mut visit = |role, block_id: &mut [u8; 16]| {
        assert_eq!(role, BlockIdRole::Existing);
        *block_id = [5; 16];
    };
    info.visit_block_ids(&mut visit);
    command.visit_block_ids(&mut visit);
    for visited in [format!("{info:?}"), format!("{command:?}")] {
        assert!(!visited.contains("[1, 1, 1"), "{visited}");
    }
}
