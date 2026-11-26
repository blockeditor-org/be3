use be_lib_geometry::vec2;
use be_lib_ui_types::input::ScrollDelta;

use super::*;

#[test]
fn a_scroll_reaches_a_zoomed_surface_in_its_own_pixels() {
    // The editor drawn at half its size: a window point covers two of its pixels.
    let mut test = canvas_with_editor();
    test.last().scale = 2.0;
    let mut router = Router::new(ROOT);
    router.route(&test, Input::PointerMoved(pos2(250.0, 250.0)));
    let scrolled = router.route(&test, Input::Scroll(ScrollDelta::Pixels(vec2(0.0, 4.0))));
    assert_eq!(
        inputs_for(&scrolled, EDITOR),
        vec![Input::Scroll(ScrollDelta::Pixels(vec2(0.0, 8.0)))],
        "pixels scrolled are the surface's own"
    );
    let notched = router.route(&test, Input::Scroll(ScrollDelta::Lines(vec2(0.0, 1.0))));
    assert_eq!(
        inputs_for(&notched, EDITOR),
        vec![Input::Scroll(ScrollDelta::Lines(vec2(0.0, 1.0)))],
        "a wheel's notches are notches however the surface is drawn"
    );
    let swiped = router.route(
        &test,
        Input::Touchpad(TouchpadGesture::Swiped(vec2(3.0, 0.0))),
    );
    assert_eq!(
        inputs_for(&swiped, EDITOR),
        vec![Input::Touchpad(TouchpadGesture::Swiped(vec2(6.0, 0.0)))]
    );
}
