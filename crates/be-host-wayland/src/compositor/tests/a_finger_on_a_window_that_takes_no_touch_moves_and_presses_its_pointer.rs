use super::*;
use beui::{TouchPhase, vec2};

#[test]
fn a_finger_on_a_window_that_takes_no_touch_moves_and_presses_its_pointer() {
    let mut harness = Harness::new();
    harness.attach_touchscreen(true);
    let _pointer = harness.client.pointer();
    let (_window, id) = harness.open();
    let at = harness.window(id).center();

    harness.frame(vec![finger(0, TouchPhase::Start, at)]);
    harness.frame(vec![finger(1, TouchPhase::Start, at + vec2(40.0, 0.0))]);
    harness.frame(vec![finger(0, TouchPhase::Move, at + vec2(10.0, 0.0))]);
    harness.frame(vec![
        finger(1, TouchPhase::End, at + vec2(40.0, 0.0)),
        finger(0, TouchPhase::End, at + vec2(10.0, 0.0)),
    ]);
    harness.settle();

    assert!(
        harness.client.received.pointer_entered.is_some(),
        "the finger moves the window's pointer onto it"
    );
    assert_eq!(
        harness.client.received.buttons,
        vec![(0x110, true), (0x110, false)],
        "the first finger down and up is a left click, and a second finger is not another"
    );
}
