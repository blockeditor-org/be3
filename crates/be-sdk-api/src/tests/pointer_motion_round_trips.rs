use super::*;

#[test]
fn pointer_motion_round_trips() {
    let message = InputBatch {
        screen: ScreenId(1),
        every_sample: false,
        dismissed: Vec::new(),
        events: vec![Input::PointerMotion(Vec2::new(-3.5, 7.25)).into()],
    };
    round_trips(&message);
}
