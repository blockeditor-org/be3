use super::*;

#[test]
fn block_content_and_operations_round_trip() {
    for editor in [
        EditorMessage::Content {
            instance: EditorInstanceId(7),
            content_type: [9; 16],
            bytes: vec![1, 2, 3, 4, 5, 6, 7, 8],
            applied: 3,
        },
        EditorMessage::Content {
            instance: EditorInstanceId(7),
            content_type: [9; 16],
            bytes: Vec::new(),
            applied: 0,
        },
        EditorMessage::Operate {
            instance: EditorInstanceId(7),
            operation: vec![0, 1],
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
