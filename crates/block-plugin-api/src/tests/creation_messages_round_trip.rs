use super::*;

#[test]
fn creation_messages_round_trip() {
    for message in [Message::Editor(EditorMessage::OpenCreation {
        instance: EditorInstanceId(2),
        editor: [1; 16],
        template: "title-slide".into(),
        account_id: [3; 16],
        organization_id: [4; 16],
        client_id: [5; 16],
    })] {
        assert_eq!(
            decode_frame(&encode_frame(&message).unwrap()).unwrap(),
            message
        );
    }
}
