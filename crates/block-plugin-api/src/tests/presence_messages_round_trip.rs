use super::*;

#[test]
fn presence_messages_round_trip() {
    let message = Message::Editor(EditorMessage::Presence {
        instance: EditorInstanceId(6),
        visible: true,
    });
    assert_eq!(
        decode_frame(&encode_frame(&message).unwrap()).unwrap(),
        message
    );
}
