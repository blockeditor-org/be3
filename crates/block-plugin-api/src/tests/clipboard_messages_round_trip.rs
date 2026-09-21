use super::*;

#[test]
fn clipboard_messages_round_trip() {
    let messages = [
        request(12, 3, HostRequest::PasteImage),
        reply(
            12,
            3,
            HostReply::ImagePasted(ClipboardImage::Pasted {
                name: "Pasted Image.png".into(),
                data: vec![1, 2, 3],
            }),
        ),
        reply(12, 4, HostReply::ImagePasted(ClipboardImage::Empty)),
        Message::Editor(EditorMessage::PasteText {
            instance: EditorInstanceId(12),
        }),
        Message::Input(InputBatch {
            screen: ScreenId(1),
            events: vec![InputEvent::Paste("pasted".into())],
        }),
    ];
    for message in messages {
        assert_eq!(
            decode_frame(&encode_frame(&message).unwrap()).unwrap(),
            message
        );
    }
}
