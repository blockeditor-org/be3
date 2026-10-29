use super::*;
use beui_core::input::ScrollDelta;
use beui_runner::{SCROLL_LINE, SCROLL_PAGE};

#[test]
fn platform_scrolls_arrive_in_points() {
    let mut intaken = Intaken::new();
    let seen = intaken.frame(
        2.0,
        [
            Input::Scroll(ScrollDelta::Pixels(vec2(0.0, 30.0))).into(),
            Input::Scroll(ScrollDelta::Lines(vec2(0.0, -1.0))).into(),
            Input::Scroll(ScrollDelta::Pages(vec2(1.0, 0.0))).into(),
            Input::Scroll(ScrollDelta::Lines(Vec2::ZERO)).into(),
        ],
    );
    assert_eq!(
        seen,
        [
            Event::Scroll(vec2(0.0, 15.0)),
            Event::Scroll(vec2(0.0, -SCROLL_LINE)),
            Event::Scroll(vec2(SCROLL_PAGE, 0.0)),
        ]
    );
}
