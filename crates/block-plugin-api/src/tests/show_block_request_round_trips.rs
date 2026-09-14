use super::*;

#[test]
fn show_block_request_round_trips() {
    for via in [Some([3; 16]), None] {
        let message = Message::Editor(EditorMessage::ShowBlock {
            instance: EditorInstanceId(3),
            block_id: [1; 16],
            block_type: [2; 16],
            via,
        });
        assert_eq!(
            decode_frame(&encode_frame(&message).unwrap()).unwrap(),
            message
        );
    }
}
