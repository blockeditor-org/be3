use super::*;

#[test]
fn zoom_gesture_round_trips() {
    let message = Message::Input(InputBatch {
        screen: ScreenId(3),
        every_sample: false,
        events: vec![Input::Zoom(1.25).into()],
    });

    assert_eq!(
        decode_frame(&encode_frame(&message).unwrap()).unwrap(),
        message
    );
}
