use super::*;

#[test]
fn a_safari_gesture_zooms_by_the_change_in_its_scale() {
    let mut pinch = Pinch::default();
    assert_eq!(
        pinch.change(2.0, false),
        None,
        "a change before the gesture starts is ignored"
    );
    pinch.start();
    assert_eq!(pinch.change(1.5, false), Some(1.5));
    assert_eq!(
        pinch.change(3.0, false),
        Some(2.0),
        "each change zooms by its scale over the last one's"
    );
    assert_eq!(pinch.change(3.0, false), None);
    assert_eq!(
        wheel(vec2(0.0, 5.0), 0, true, &pinch),
        None,
        "a ctrl-wheel during a gesture is the same pinch and is not zoomed twice"
    );
    pinch.end();
    assert_eq!(pinch.change(4.0, false), None);
}
