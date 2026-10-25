use super::*;

use crate::StoredHead;

#[test]
fn head_messages_round_trip() {
    for message in [
        Message::Editor(EditorMessage::WatchHeads {
            instance: EditorInstanceId(3),
            blocks: vec![[1; 16]],
        }),
        Message::Editor(EditorMessage::Head {
            instance: EditorInstanceId(3),
            block_id: [1; 16],
            head: Ok(Some(StoredHead {
                commit: [2; 32],
                content_type: [4; 16],
                bytes: vec![0xa1, 1, 2],
            })),
        }),
        Message::Editor(EditorMessage::Head {
            instance: EditorInstanceId(3),
            block_id: [1; 16],
            head: Ok(None),
        }),
        Message::Editor(EditorMessage::Head {
            instance: EditorInstanceId(3),
            block_id: [1; 16],
            head: Err("unreadable".into()),
        }),
    ] {
        assert_eq!(
            decode_frame(&encode_frame(&message).unwrap()).unwrap(),
            message
        );
    }
}
