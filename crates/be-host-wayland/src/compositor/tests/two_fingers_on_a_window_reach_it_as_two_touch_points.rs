use super::*;
use crate::test_client::TouchSeen;
use beui::{TouchPhase, vec2};

#[test]
fn two_fingers_on_a_window_reach_it_as_two_touch_points() {
    let mut harness = Harness::new();
    harness.attach_touchscreen(true);
    let _touch = harness.client.touch();
    let _pointer = harness.client.pointer();
    let (window, id) = harness.open();
    let shown = harness.window(id);
    let left = shown.center() - vec2(50.0, 0.0);
    let right = shown.center() + vec2(50.0, 0.0);
    let local = |pos: Pos2| {
        (
            f64::from(pos.x - shown.min.x),
            f64::from(pos.y - shown.min.y),
        )
    };

    harness.frame(vec![
        finger(0, TouchPhase::Start, left),
        finger(1, TouchPhase::Start, right),
    ]);
    let apart = vec2(20.0, 0.0);
    harness.frame(vec![
        finger(0, TouchPhase::Move, left - apart),
        finger(1, TouchPhase::Move, right + apart),
    ]);
    harness.frame(vec![
        finger(0, TouchPhase::End, left - apart),
        finger(1, TouchPhase::End, right + apart),
    ]);
    harness.settle();

    let (left_x, left_y) = local(left);
    let (right_x, right_y) = local(right);
    let (wide_left, _) = local(left - apart);
    let (wide_right, _) = local(right + apart);
    assert_eq!(
        harness.client.received.protocols.touch,
        [
            TouchSeen::Down(window.surface.clone(), 0, left_x, left_y),
            TouchSeen::Down(window.surface.clone(), 1, right_x, right_y),
            TouchSeen::Frame,
            TouchSeen::Motion(0, wide_left, left_y),
            TouchSeen::Motion(1, wide_right, right_y),
            TouchSeen::Frame,
            TouchSeen::Up(0),
            TouchSeen::Up(1),
            TouchSeen::Frame,
        ],
        "both fingers reach the window as touch points of their own, in surface coordinates"
    );
    assert!(
        harness.client.received.buttons.is_empty(),
        "a window that takes touch hears nothing of the fingers as a pointer"
    );
}
