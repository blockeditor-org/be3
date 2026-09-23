use super::*;

#[test]
fn block_content_and_operations_round_trip() {
    for editor in [
        EditorMessage::Content {
            instance: EditorInstanceId(7),
            block_id: [4; 16],
            content_type: [9; 16],
            bytes: vec![1, 2, 3, 4, 5, 6, 7, 8],
            applied: 3,
        },
        EditorMessage::Content {
            instance: EditorInstanceId(7),
            block_id: [4; 16],
            content_type: [9; 16],
            bytes: Vec::new(),
            applied: 0,
        },
        EditorMessage::ContentOperations {
            instance: EditorInstanceId(7),
            block_id: [5; 16],
            operations: vec![ContentOperation {
                operation: vec![3],
                mine: true,
            }],
        },
        EditorMessage::Operate {
            instance: EditorInstanceId(7),
            block_id: [5; 16],
            operation: vec![0, 1],
        },
        EditorMessage::WatchContent {
            instance: EditorInstanceId(7),
            blocks: vec![WatchedContent {
                block_id: [5; 16],
                content_type: [9; 16],
            }],
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
