use super::*;

#[test]
fn multiplexed_messages_round_trip() {
    let editor = Message::Editor(EditorMessage::Close {
        instance: EditorInstanceId(7),
    });
    let client = Message::Editor(EditorMessage::Blocks {
        instance: EditorInstanceId(7),
        query: BlockQuery::Roots,
        blocks: vec![BlockInfo {
            block_id: [1; 16],
            block_type: [2; 16],
            author: [3; 16],
            parent: BlockLocation::Root,
            name: Some("Notes".to_owned()),
            named_by_hand: true,
            references: vec![[4; 16]],
            access: AccessLevel::Edit,
            artifact: None,
            thumbhash: Some(Thumbhash {
                hash: vec![5, 6, 7],
                width: 640,
                height: 480,
            }),
        }],
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
