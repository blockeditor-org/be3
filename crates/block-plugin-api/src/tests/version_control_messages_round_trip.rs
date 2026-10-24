use super::*;

use crate::{
    ConflictSide, VersionBranch, VersionChange, VersionChangeKind, VersionCommand, VersionCommit,
    VersionOperation, VersionStatus,
};

#[test]
fn version_control_messages_round_trip() {
    for message in [
        Message::Editor(EditorMessage::VersionControl {
            instance: EditorInstanceId(4),
            block_id: [1; 16],
            command: VersionCommand::Finish {
                description: "First draft".into(),
            },
        }),
        Message::Editor(EditorMessage::VersionControl {
            instance: EditorInstanceId(4),
            block_id: [1; 16],
            command: VersionCommand::Resolve {
                block_id: [2; 16],
                take: ConflictSide::Theirs,
            },
        }),
        Message::Editor(EditorMessage::VersionControl {
            instance: EditorInstanceId(4),
            block_id: [1; 16],
            command: VersionCommand::Undo { operation: [3; 16] },
        }),
        Message::Editor(EditorMessage::VersionStatus {
            instance: EditorInstanceId(4),
            block_id: [1; 16],
            status: VersionStatus {
                busy: true,
                error: Some("the branch moved".into()),
                behind: true,
                branches: vec![VersionBranch {
                    name: "main".into(),
                    head: [7; 32],
                }],
                changes: vec![VersionChange {
                    block_id: [2; 16],
                    block_type: [3; 16],
                    name: Some("Notes".into()),
                    kind: VersionChangeKind::Modified,
                }],
                working: Some(VersionCommit {
                    id: [8; 32],
                    change: [5; 16],
                    parents: vec![[7; 32]],
                    author: [9; 16],
                    time: 2_000,
                    message: String::new(),
                    conflicted: true,
                }),
                log: vec![VersionCommit {
                    id: [7; 32],
                    change: [4; 16],
                    parents: vec![[6; 32]],
                    author: [9; 16],
                    time: 1_000,
                    message: "First draft".into(),
                    conflicted: false,
                }],
                operations: vec![VersionOperation {
                    id: [3; 16],
                    time: 1_000,
                    author: [9; 16],
                    description: "Saved the change".into(),
                    undone: false,
                }],
            },
        }),
    ] {
        assert_eq!(
            decode_frame(&encode_frame(&message).unwrap()).unwrap(),
            message
        );
    }
}
