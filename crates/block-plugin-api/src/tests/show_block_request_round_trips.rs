use super::*;

#[test]
fn show_block_request_round_trips() {
    for (via, from) in [(Some([3; 16]), Some([4; 16])), (None, None)] {
        let message = Message::Editor(EditorMessage::ShowBlock {
            instance: EditorInstanceId(3),
            block_id: [1; 16],
            block_type: [2; 16],
            via,
            from,
        });
        assert_eq!(
            decode_frame(&encode_frame(&message).unwrap()).unwrap(),
            message
        );
    }
}
