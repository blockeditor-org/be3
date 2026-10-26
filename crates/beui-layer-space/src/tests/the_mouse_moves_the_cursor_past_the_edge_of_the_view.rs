use super::*;
use crate::head::MOST_ELEVATION;

#[test]
fn the_mouse_moves_the_cursor_past_the_edge_of_the_view() {
    let (mut space, now) = space();
    send(&mut space, now, [Input::PointerMoved(centre())]);
    send(&mut space, now, [Input::PointerMotion(vec2(30.0, 20.0))]);
    let start = space.cursor_screen().unwrap();
    assert!(
        (start - (centre() + vec2(30.0, 20.0))).length() < 0.1,
        "near the middle, motion moves the cursor as far across the screen: {start:?}"
    );

    send(&mut space, now, [Input::PointerMotion(vec2(500.0, 0.0))]);
    assert!(
        space.cursor_screen().unwrap().x > PLATFORM.x,
        "nothing holds it at the edge"
    );
    assert_eq!(space.cursor_at(), None);
    let (_, angle) = space.cursor_away().expect("a marker shows instead");
    assert!(
        (angle - std::f32::consts::FRAC_PI_2).abs() < 0.1,
        "pointing right: {angle}"
    );

    send(&mut space, now, [Input::PointerMotion(vec2(-500.0, 0.0))]);
    assert!(
        (space.cursor_screen().unwrap() - start).length() < 0.1,
        "moving back brings it back where it was"
    );

    send(
        &mut space,
        now,
        [Input::PointerMotion(vec2(0.0, -100_000.0))],
    );
    let up = space.cursor_ray().unwrap().direction.y;
    assert!(
        (up - MOST_ELEVATION.sin()).abs() < 1e-3,
        "it stops short of straight up: {up}"
    );
}
