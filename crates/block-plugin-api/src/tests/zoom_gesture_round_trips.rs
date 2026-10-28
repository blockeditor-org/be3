use super::*;

#[test]
fn zoom_gesture_round_trips() {
    let message = InputBatch {
        screen: ScreenId(3),
        every_sample: false,
        dismissed: Vec::new(),
        events: vec![Input::Zoom(1.25).into()],
    };

    round_trips(&message);
}
