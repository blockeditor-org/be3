use super::*;

#[test]
fn block_content_and_operations_round_trip() {
    for editor in [
        EditorMessage::WatchContent {
            instance: EditorInstanceId(7),
            blocks: vec![WatchedContent {
                block_id: [5; 16],
                content_type: [9; 16],
            }],
        },
        EditorMessage::SeedContent {
            instance: EditorInstanceId(7),
            block_id: [5; 16],
            content_type: [9; 16],
            bytes: vec![1, 2],
        },
        EditorMessage::ReplaceContent {
            instance: EditorInstanceId(7),
            block_id: [5; 16],
            content_type: [9; 16],
            bytes: vec![3],
        },
    ] {
        assert_eq!(editor.instance(), EditorInstanceId(7));
        let message = Message::Editor(editor);
        assert_eq!(
            decode_frame(&encode_frame(&message).unwrap()).unwrap(),
            message
        );
    }
}
