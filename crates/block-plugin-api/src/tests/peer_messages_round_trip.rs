use super::*;

#[test]
fn peer_messages_round_trip() {
    for message in [
        EditorMessage::SendToPeers {
            instance: EditorInstanceId(3),
            block_id: [7; 16],
            to: Some(12),
            channel: [9; 16],
            payload: vec![1, 2, 3],
        },
        EditorMessage::PeerMessages {
            instance: EditorInstanceId(3),
            block_id: [7; 16],
            messages: vec![PeerMessage {
                from: 12,
                channel: [9; 16],
                payload: b"hello".to_vec(),
            }],
        },
    ] {
        let message = Message::Editor(message);
        assert_eq!(
            decode_frame(&encode_frame(&message).unwrap()).unwrap(),
            message
        );
    }
}
