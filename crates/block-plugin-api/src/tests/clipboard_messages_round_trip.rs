use super::*;

#[test]
fn clipboard_messages_round_trip() {
    let messages = [
        Message::Editor(EditorMessage::Paste {
            instance: EditorInstanceId(12),
        }),
        Message::Editor(EditorMessage::CopyImage {
            instance: EditorInstanceId(12),
            png: vec![137, 80, 78, 71],
        }),
        Message::Input(InputBatch {
            screen: ScreenId(1),
            every_sample: false,
            events: vec![
                InputEvent::Paste(Pasted::Text("pasted".into())),
                InputEvent::Paste(Pasted::Image(PickedFile {
                    name: "Pasted Image.png".into(),
                    data: vec![1, 2, 3],
                })),
            ],
        }),
    ];
    for message in messages {
        assert_eq!(
            decode_frame(&encode_frame(&message).unwrap()).unwrap(),
            message
        );
    }
}
