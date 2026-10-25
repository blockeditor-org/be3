use super::*;

#[test]
fn an_oversized_block_filter_is_refused() {
    let oversized = Message::Editor(EditorMessage::PickRequested {
        instance: EditorInstanceId(2),
        pick: 8,
        filter: BlockFilter {
            name: "x".repeat(MAX_STRING_BYTES + 1),
            ..BlockFilter::default()
        },
        parent: BlockLocation::Root,
        within: None,
        touch: false,
    });
    assert_eq!(
        encode_frame(&oversized),
        Err(DecodeError::LimitExceeded("string"))
    );
}
