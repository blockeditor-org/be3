use super::*;

#[test]
fn a_gesture_made_with_fingers_on_the_screen_is_left_to_the_touches() {
    let mut pinch = Pinch::default();
    pinch.start();
    assert_eq!(
        pinch.change(2.0, true),
        None,
        "touches already pinch, so iOS's gesture events do not zoom again"
    );
    assert_eq!(pinch.change(4.0, false), Some(2.0));
}
