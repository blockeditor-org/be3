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
                author: Some([9; 16]),
                parent: Some(BlockLocation::Block([1; 16])),
                references: vec![[1; 16]],
                access: AccessLevel::Edit,
                metadata: crate::BlockMetadata::default(),
                locked: false,
                deleted: false,
                sets_access: false,
            }],
        }),
        Message::Editor(EditorMessage::Replied {
            instance: EditorInstanceId(2),
            request_id: 3,
            reply: crate::HostReply::AccessListed(crate::AccessListing::Listed {
                grants: vec![crate::AccessGrant {
                    account: [9; 16],
                    email: String::new(),
                    display_name: String::new(),
                    administrator: false,
                    author: false,
                    granted: None,
                    inherited: AccessLevel::None,
                    effective: AccessLevel::Edit,
                    source: Some([1; 16]),
                    waiting: false,
                }],
                company: false,
                set_inside: 0,
            }),
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
        defined: crate::MetadataEntries::new(),
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
