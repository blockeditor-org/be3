use super::*;
use beui_core::runner::{Input, PINCH_SPEED, ScrollDelta};

#[test]
fn logical_input_arrives_in_points_at_the_zoom_of_the_frame() {
    let mut intaken = Intaken::new();
    intaken.runner.context().set_zoom_factor(2.0);
    let seen = intaken.frame(
        2.0,
        [
            Input::logical(Event::PointerMoved(pos2(10.0, 20.0))),
            Input::logical(Event::PointerMotion(vec2(4.0, -8.0))),
            Input::logical(ScrollDelta::Pixels(vec2(0.0, 30.0))),
            Input::logical(Input::Pinch(ScrollDelta::Pixels(vec2(0.0, 30.0)))),
            Event::PointerMoved(pos2(10.0, 20.0)).into(),
        ],
    );
    assert_eq!(
        seen,
        [
            Event::PointerMoved(pos2(5.0, 10.0)),
            Event::PointerMotion(vec2(2.0, -4.0)),
            Event::Scroll(vec2(0.0, 15.0)),
            Event::Zoom((15.0 * PINCH_SPEED).exp()),
            Event::PointerMoved(pos2(2.5, 5.0)),
        ],
        "a logical pixel is the window's scale in physical pixels, and a point is the zoomed scale"
    );
}
