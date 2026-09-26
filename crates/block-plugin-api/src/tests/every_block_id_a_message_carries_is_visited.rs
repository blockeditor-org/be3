use super::*;

use crate::{AccessLevel, BlockIdRole, BlockInfo, BlockLocation, BlockQuery, VersionCommand};

#[test]
fn every_block_id_a_message_carries_is_visited() {
    let mut messages = [
        Message::Editor(EditorMessage::Blocks {
            instance: EditorInstanceId(2),
            query: BlockQuery::Children([1; 16]),
            blocks: vec![BlockInfo {
                block_id: [1; 16],
                block_type: [9; 16],
                author: [9; 16],
                parent: BlockLocation::Block([1; 16]),
                name: None,
                named_by_hand: false,
                references: vec![[1; 16]],
                access: AccessLevel::Edit,
                artifact: None,
            }],
        }),
        Message::Editor(EditorMessage::VersionControl {
            instance: EditorInstanceId(2),
            block_id: [1; 16],
            command: VersionCommand::Adopt { block_id: [1; 16] },
        }),
    ];
    for message in &mut messages {
        message.visit_block_ids(&mut |instance, role, block_id| {
            assert_eq!(instance, EditorInstanceId(2));
            assert_eq!(role, BlockIdRole::Existing);
            *block_id = [5; 16];
        });
        let encoded = format!("{message:?}");
        assert!(!encoded.contains("[1, 1, 1"), "{encoded}");
    }

    let mut created = Message::Editor(EditorMessage::CreateBlock {
        instance: EditorInstanceId(2),
        block_id: [1; 16],
        content_type: [9; 16],
        parent: BlockLocation::Block([4; 16]),
        name: None,
        artifact: None,
        content: None,
    });
    let mut roles = Vec::new();
    created.visit_block_ids(&mut |_, role, block_id| roles.push((role, *block_id)));
    assert_eq!(
        roles,
        vec![
            (BlockIdRole::Existing, [4; 16]),
            (BlockIdRole::Created, [1; 16]),
        ]
    );
}
