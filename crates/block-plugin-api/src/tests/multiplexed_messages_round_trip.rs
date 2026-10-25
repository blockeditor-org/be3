use super::*;

#[test]
fn multiplexed_messages_round_trip() {
    let editor = Message::Editor(EditorMessage::Close {
        instance: EditorInstanceId(7),
    });
    let client = Message::Editor(EditorMessage::OpenCreation {
        instance: EditorInstanceId(7),
        editor: [1; 16],
        template: "main".to_owned(),
        account_id: [2; 16],
        organization_id: [3; 16],
        client_id: [4; 16],
    });
    assert_eq!(
        decode_frame(&encode_frame(&editor).unwrap()).unwrap(),
        editor
    );
    assert_eq!(
        decode_frame(&encode_frame(&client).unwrap()).unwrap(),
        client
    );
}
