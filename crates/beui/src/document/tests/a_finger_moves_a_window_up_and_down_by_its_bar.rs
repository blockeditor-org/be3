use super::*;
use crate::unstyled::dock_state;

#[test]
fn a_finger_moves_a_window_up_and_down_by_its_bar() {
    let (document, dock) = dock_of(2);
    let mut harness = Harness::sized(document, WIDE_VIEWPORT);
    let window = floated_window(&mut harness, dock);
    let before = dock_state(harness.document(), dock)
        .window_rect(window)
        .expect("the window has a rect");
    let origin = harness.rect(dock).min + before.min.to_vec2();
    let bar = pos2(origin.x + before.width() - 80.0, origin.y + 10.0);

    harness.finger_drag(&[bar, bar + vec2(0.0, -20.0), bar + vec2(-30.0, -100.0)]);
    harness.frame(Vec::new());

    let after = dock_state(harness.document(), dock)
        .window_rect(window)
        .expect("the window is still open");
    assert_eq!(
        after.min,
        before.min + vec2(-30.0, -100.0),
        "a finger dragging the bar upwards moves the window rather than being taken for a scroll"
    );
}
