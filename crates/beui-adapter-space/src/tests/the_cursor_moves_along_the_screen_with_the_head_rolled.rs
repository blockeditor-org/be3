use super::*;

#[test]
fn the_cursor_moves_along_the_screen_with_the_head_rolled() {
    let (mut space, now) = space();
    space.head.roll = 0.5;
    send(&mut space, now, [Input::PointerMoved(centre())]);
    let start = space.cursor_screen().unwrap();
    send(&mut space, now, [Input::PointerMotion(vec2(60.0, 0.0))]);
    let moved = space.cursor_screen().unwrap() - start;
    assert!(
        (moved - vec2(60.0, 0.0)).length() < 0.5,
        "motion to the right moves it right on the screen: {moved:?}"
    );
    send(&mut space, now, [Input::PointerMotion(vec2(0.0, 40.0))]);
    let moved = space.cursor_screen().unwrap() - start;
    assert!(
        (moved - vec2(60.0, 40.0)).length() < 0.5,
        "and down moves it down: {moved:?}"
    );
}
