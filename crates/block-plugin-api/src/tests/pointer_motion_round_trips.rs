use super::*;

#[test]
fn pointer_motion_round_trips() {
    let message = Message::Input(InputBatch {
        screen: ScreenId(1),
        every_sample: false,
        events: vec![Input::PointerMotion(Vec2::new(-3.5, 7.25)).into()],
    });
    assert_eq!(
        decode_frame(&encode_frame(&message).unwrap()).unwrap(),
        message
    );
}
