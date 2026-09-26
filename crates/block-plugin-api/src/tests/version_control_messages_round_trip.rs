use super::*;

use crate::{
    ConflictSide, VersionBranch, VersionChange, VersionChangeKind, VersionCommand, VersionCommit,
    VersionStatus,
};

#[test]
fn version_control_messages_round_trip() {
    for message in [
        Message::Editor(EditorMessage::VersionControl {
            instance: EditorInstanceId(4),
            block_id: [1; 16],
            command: VersionCommand::Commit {
                message: "First draft".into(),
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
                log: vec![VersionCommit {
                    id: [7; 32],
                    parents: vec![[6; 32]],
                    author: [9; 16],
                    time: 1_000,
                    message: "First draft".into(),
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
