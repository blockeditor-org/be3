use super::*;

use crate::test_client::Feedback;

#[test]
fn presentation_feedback_carries_the_flip_time() {
    let (mut harness, device, queue) = Harness::with_gpu();
    harness.compositor().pace_to_flips();
    let (window, _) = harness.open();
    let mut painter = Painter::new(&device, &queue);

    let _feedback = harness.client.presentation_feedback(&window.surface);
    harness.client.attach_unsent(&window, 300, 200);
    harness.frame(Vec::new());
    painter.paint(&harness);
    harness.frame(Vec::new());
    assert_eq!(
        harness.client.received.presented,
        Vec::new(),
        "a painted frame is not presented before its screen flips"
    );

    let flipped = Presented {
        time: Duration::new(1234, 567_000_000),
        ..flip(0)
    };
    harness
        .compositor()
        .presented(Rect::from_min_size(Pos2::ZERO, SCREEN), flipped);
    harness.frame(Vec::new());
    assert_eq!(harness.client.received.clock, Some(1), "CLOCK_MONOTONIC");
    let vsync = 0x1;
    let hardware_clock = 0x2;
    let hardware_completion = 0x4;
    assert_eq!(
        harness.client.received.presented,
        vec![Feedback {
            time: Duration::new(1234, 567_000_000),
            refresh: 16_666_667,
            sequence: 0,
            flags: vsync | hardware_clock | hardware_completion,
        }]
    );
    assert_eq!(harness.client.received.discarded, 0);
}
