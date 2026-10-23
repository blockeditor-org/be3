use super::*;

#[test]
fn a_stopped_region_and_pointer_motion_round_trip() {
    for message in [
        Message::Editor(EditorMessage::Stopped {
            instance: EditorInstanceId(4),
            region: EditorRegion::Frame,
            stopped: Stopped::Failed("out of cheese".to_owned()),
        }),
        Message::Editor(EditorMessage::Stopped {
            instance: EditorInstanceId(4),
            region: EditorRegion::Preview,
            stopped: Stopped::Closed,
        }),
        Message::Input(InputBatch {
            screen: ScreenId(1),
            every_sample: false,
            events: vec![Input::PointerMotion(Vec2::new(-3.5, 7.25)).into()],
        }),
    ] {
        assert_eq!(
            decode_frame(&encode_frame(&message).unwrap()).unwrap(),
            message
        );
    }
}
