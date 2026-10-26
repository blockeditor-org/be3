use super::*;
use crate::CURSOR_MARGIN;

#[test]
fn moving_the_mouse_from_the_edge_starts_where_the_cursor_is_drawn() {
    let (mut space, now) = space();
    send(&mut space, now, [Input::PointerMoved(centre())]);
    send(&mut space, now, [Input::PointerMotion(vec2(30.0, 20.0))]);
    assert!(
        (space.cursor_screen().unwrap() - (centre() + vec2(30.0, 20.0))).length() < 0.01,
        "motion moves the cursor as far across the screen"
    );

    send(&mut space, now, steer(true));
    send(&mut space, now, [Input::PointerMotion(vec2(-500.0, 0.0))]);
    send(&mut space, now, steer(false));
    let edge = space.cursor_screen().unwrap();
    assert!(
        (edge.x - (PLATFORM.x - CURSOR_MARGIN)).abs() < 0.01,
        "turned left, it is drawn at the right edge: {edge:?}"
    );

    send(&mut space, now, [Input::PointerMotion(vec2(-40.0, 0.0))]);
    let moved = space.cursor_screen().unwrap();
    assert!(
        (moved - (edge + vec2(-40.0, 0.0))).length() < 0.01,
        "the mouse moves it from where it is drawn: {moved:?}"
    );

    send(&mut space, now, steer(true));
    send(&mut space, now, [Input::PointerMotion(vec2(500.0, 0.0))]);
    send(&mut space, now, steer(false));
    let back = space.cursor_screen().unwrap();
    assert!(
        back.x < PLATFORM.x - 100.0,
        "and that is its new point, which turning back brings into view: {back:?}"
    );

    send(&mut space, now, [Input::PointerMotion(vec2(10_000.0, 0.0))]);
    assert!(
        (space.cursor_screen().unwrap().x - (PLATFORM.x - CURSOR_MARGIN)).abs() < 0.01,
        "the mouse moves it no further than the edge"
    );
}
