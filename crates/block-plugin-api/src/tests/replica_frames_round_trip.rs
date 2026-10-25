use super::*;

#[test]
fn replica_frames_round_trip() {
    let mut message = Message::Editor(EditorMessage::Replica {
        instance: EditorInstanceId(4),
        block_id: [1; 16],
        frame: vec![0, 0, 0, 2, 7, 9],
    });
    assert_eq!(message.direction(), Direction::Either);
    assert_eq!(
        decode_frame(&encode_frame(&message).unwrap()).unwrap(),
        message
    );
    let mut visited = Vec::new();
    message.visit_block_ids(&mut |instance, role, block_id| {
        visited.push((instance, role, *block_id));
        *block_id = [5; 16];
    });
    assert_eq!(
        visited,
        vec![(EditorInstanceId(4), crate::BlockIdRole::Existing, [1; 16])]
    );
    let Message::Editor(EditorMessage::Replica { block_id, .. }) = message else {
        panic!("{message:?}");
    };
    assert_eq!(block_id, [5; 16]);
}
