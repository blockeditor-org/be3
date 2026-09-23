use super::*;

use crate::{BlockCommand, HistoryState};

#[test]
fn history_messages_round_trip() {
    for message in [
        Message::Editor(EditorMessage::WatchHistory {
            instance: EditorInstanceId(3),
            blocks: vec![[1; 16], [2; 16]],
        }),
        Message::Editor(EditorMessage::HistoryStates {
            instance: EditorInstanceId(3),
            states: vec![HistoryState {
                block_id: [1; 16],
                can_undo: true,
                can_redo: false,
            }],
        }),
        Message::Editor(EditorMessage::BlockCommand {
            instance: EditorInstanceId(3),
            block_id: [1; 16],
            command: BlockCommand::Undo,
        }),
    ] {
        assert_eq!(
            decode_frame(&encode_frame(&message).unwrap()).unwrap(),
            message
        );
    }
}
