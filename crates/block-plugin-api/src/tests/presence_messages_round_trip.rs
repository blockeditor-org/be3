use super::*;

#[test]
fn presence_messages_round_trip() {
    for message in [
        Message::Editor(EditorMessage::Presence {
            instance: EditorInstanceId(6),
            visible: true,
        }),
        Message::Editor(EditorMessage::RevealPresence {
            instance: EditorInstanceId(6),
            client_id: 42,
        }),
    ] {
        assert_eq!(
            decode_frame(&encode_frame(&message).unwrap()).unwrap(),
            message
        );
    }
}
