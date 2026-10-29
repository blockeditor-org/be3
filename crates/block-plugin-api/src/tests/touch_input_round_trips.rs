use super::*;

#[test]
fn touch_input_round_trips() {
    let message = InputBatch {
        screen: ScreenId(3),
        every_sample: false,
        dismissed: Vec::new(),
        events: vec![TimedInput {
            input: Input::Touch {
                id: TouchId {
                    device: 4,
                    finger: 5,
                },
                phase: TouchPhase::Move,
                pos: Pos2::new(12.0, 24.0),
                force: Some(0.75),
            },
            time_micros: Some(1_234_567),
            over: Some(ScreenId(4)),
            anchored: Some(ViewId(2)),
        }],
    };

    round_trips(&message);
}
