use super::*;

#[test]
fn a_plain_wheel_scrolls_and_a_ctrl_wheel_pinches() {
    let pinch = Pinch::default();
    assert_eq!(
        wheel(vec2(3.0, 4.0), 0, false, &pinch),
        Some(Input::Scroll(ScrollDelta::Pixels(vec2(-3.0, -4.0)))),
        "a trackpad's two-finger scroll pans by the pixels it moved"
    );
    assert_eq!(
        wheel(vec2(0.0, 2.0), DOM_DELTA_LINE, false, &pinch),
        Some(Input::Scroll(ScrollDelta::Lines(vec2(0.0, -2.0)))),
        "a mouse wheel scrolls by lines"
    );
    assert_eq!(
        wheel(vec2(0.0, 5.0), 0, true, &pinch),
        Some(Input::Zoom((-5.0 * PINCH_SPEED).exp())),
        "browsers report a trackpad pinch as a wheel with ctrlKey"
    );
}
