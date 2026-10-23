use super::*;

#[test]
fn rejects_collection_over_limit() {
    let message = Message::Input(InputBatch {
        screen: ScreenId(1),
        every_sample: false,
        events: vec![Input::Focus(true).into(); MAX_COLLECTION_ITEMS + 1],
    });
    assert_eq!(
        encode_frame(&message),
        Err(DecodeError::LimitExceeded("collection"))
    );
}
