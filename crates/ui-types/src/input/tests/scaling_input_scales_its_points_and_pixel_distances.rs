use super::*;
use geometry::{Vec2, pos2};

#[test]
fn scaling_input_scales_its_points_and_pixel_distances() {
    assert_eq!(
        Input::PointerMoved(pos2(10.0, 20.0)).scaled(2.0),
        Input::PointerMoved(pos2(20.0, 40.0))
    );
    assert_eq!(
        Input::PointerButton {
            pos: pos2(1.5, 3.0),
            button: PointerButton::Primary,
            pressed: true,
        }
        .scaled(2.0),
        Input::PointerButton {
            pos: pos2(3.0, 6.0),
            button: PointerButton::Primary,
            pressed: true,
        }
    );
    assert_eq!(
        Input::PointerMotion(Vec2::new(4.0, -2.0)).scaled(1.5),
        Input::PointerMotion(Vec2::new(6.0, -3.0))
    );
    assert_eq!(
        Input::Scroll(ScrollDelta::Pixels(Vec2::new(0.0, 10.0))).scaled(2.0),
        Input::Scroll(ScrollDelta::Pixels(Vec2::new(0.0, 20.0)))
    );
    assert_eq!(
        Input::Scroll(ScrollDelta::Lines(Vec2::new(0.0, 3.0))).scaled(2.0),
        Input::Scroll(ScrollDelta::Lines(Vec2::new(0.0, 3.0))),
        "lines and pages are counts, not distances"
    );
    assert_eq!(Input::Zoom(1.5).scaled(2.0), Input::Zoom(1.5));
}
