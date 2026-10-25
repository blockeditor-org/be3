use super::*;

use crate::{AccessLevel, ArtifactAction, BlockCommand, BlockLocation};

#[test]
fn block_commands_round_trip() {
    for command in [
        BlockCommand::Share,
        BlockCommand::Leave,
        BlockCommand::Rename { anchor: None },
        BlockCommand::Rename {
            anchor: Some(Rect::from_min_size(
                Pos2::new(4.0, 8.0),
                Vec2::new(16.0, 24.0),
            )),
        },
        BlockCommand::Unlink { container: [5; 16] },
        BlockCommand::Delete {
            block_type: [2; 16],
            source: BlockLocation::Root,
        },
        BlockCommand::Move {
            block_type: [2; 16],
            source: BlockLocation::Block([4; 16]),
            destination: [3; 16],
            is_reference: true,
        },
        BlockCommand::Place {
            block_type: [2; 16],
            parent: [3; 16],
            linked: true,
        },
        BlockCommand::Delete {
            block_type: [2; 16],
            source: BlockLocation::Detached,
        },
        BlockCommand::Artifact {
            action: ArtifactAction::Regenerate,
        },
        BlockCommand::Artifact {
            action: ArtifactAction::Unlink,
        },
        BlockCommand::SimulateAccess {
            access: AccessLevel::View,
        },
        BlockCommand::CloseEditor,
    ] {
        let message = Message::Editor(EditorMessage::BlockCommand {
            instance: EditorInstanceId(6),
            block_id: [1; 16],
            command,
        });
        assert_eq!(
            decode_frame(&encode_frame(&message).unwrap()).unwrap(),
            message
        );
    }
}
