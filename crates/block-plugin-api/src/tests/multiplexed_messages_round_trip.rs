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
            author: Some([3; 16]),
            parent: Some(BlockLocation::Root),
            references: vec![[4; 16]],
            access: AccessLevel::Edit,
            metadata: BlockMetadata {
                defined: [([8; 16], ByteBuf::from(b"Notes".to_vec()))].into(),
                derived: [([9; 16], ByteBuf::from(vec![5, 6, 7]))].into(),
            },
            locked: false,
            deleted: false,
            sets_access: false,
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
