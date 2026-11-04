use super::*;
use crate::test_client::PointerSeen;
use beui::{TouchpadGesture, vec2};

#[test]
fn touchpad_gestures_and_pointer_motion_over_a_window_reach_it() {
    let mut harness = Harness::new();
    let pointer = harness.client.pointer();
    let _relative = harness.client.relative_pointer(&pointer);
    let _gestures = harness.client.gestures(&pointer);
    let (_window, id) = harness.open();
    let origin = harness.window(id).min;
    harness.frame(vec![Input::PointerMoved(origin + vec2(10.0, 10.0))]);
    harness.frame(vec![Input::PointerMoved(origin + vec2(14.0, 7.0))]);
    harness.settle();
    assert_eq!(
        harness.client.received.protocols.pointer,
        [PointerSeen::Relative(4.0, -3.0, 4.0, -3.0)],
        "a pointer moving over a window moves it by as much relatively"
    );

    harness.client.received.protocols.pointer.clear();
    harness.frame(
        [
            TouchpadGesture::SwipeBegan(3),
            TouchpadGesture::Swiped(vec2(4.0, 2.0)),
            TouchpadGesture::SwipeEnded(false),
            TouchpadGesture::PinchBegan(2),
            TouchpadGesture::Pinched {
                delta: vec2(1.0, -1.0),
                scale: 1.5,
                rotation: 10.0,
            },
            TouchpadGesture::PinchEnded(true),
            TouchpadGesture::HoldBegan(4),
            TouchpadGesture::HoldEnded(false),
        ]
        .into_iter()
        .map(Input::Touchpad)
        .collect(),
    );
    harness.settle();
    assert_eq!(
        harness.client.received.protocols.pointer,
        [
            PointerSeen::SwipeBegan(3),
            PointerSeen::Swiped(4.0, 2.0),
            PointerSeen::SwipeEnded(false),
            PointerSeen::PinchBegan(2),
            PointerSeen::Pinched(1.0, -1.0, 1.5, 10.0),
            PointerSeen::PinchEnded(true),
            PointerSeen::HoldBegan(4),
            PointerSeen::HoldEnded(false),
        ],
        "the window under the pointer hears each touchpad gesture"
    );
}
