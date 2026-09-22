use super::*;

#[test]
fn pick_block_messages_round_trip() {
    let asked = request(
        2,
        7,
        HostRequest::PickBlock(BlockFilter {
            name: "Slide".into(),
            block_types: vec![[9; 16]],
            excluded: vec![[3; 16]],
            templates: true,
        }),
    );
    assert_eq!(decode_frame(&encode_frame(&asked).unwrap()).unwrap(), asked);

    for pick in [
        BlockPick::Chosen {
            block_id: [1; 16],
            block_type: [9; 16],
            linked: true,
        },
        BlockPick::Cancelled,
        BlockPick::Failed("the block could not be created".into()),
    ] {
        let message = reply(2, 7, HostReply::BlockPicked(pick));
        assert_eq!(
            decode_frame(&encode_frame(&message).unwrap()).unwrap(),
            message
        );
    }

    let oversized = request(
        2,
        8,
        HostRequest::PickBlock(BlockFilter {
            name: "x".repeat(MAX_STRING_BYTES + 1),
            block_types: Vec::new(),
            excluded: Vec::new(),
            templates: false,
        }),
    );
    assert_eq!(
        encode_frame(&oversized),
        Err(DecodeError::LimitExceeded("string"))
    );
}
