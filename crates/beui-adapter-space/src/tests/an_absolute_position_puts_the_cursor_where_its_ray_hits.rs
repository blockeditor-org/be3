use super::*;
use crate::CURSOR_LIFT;

#[test]
fn an_absolute_position_puts_the_cursor_where_its_ray_hits() {
    let (mut space, now) = space();
    let at = centre() + vec2(-120.0, 80.0);
    send(&mut space, now, [Input::PointerMoved(at)]);
    let hit = main(&space)
        .hit(space.head.ray(PLATFORM, at.to_vec2()))
        .expect("the ray hits the main panel");
    let saved = space.cursor().expect("the cursor is placed");
    assert!(
        close(
            saved,
            space.head.position + (hit.point - space.head.position) * (1.0 - CURSOR_LIFT)
        ),
        "just in front of what it hits"
    );
    assert!((space.cursor_screen().unwrap() - at).length() < 0.01);

    let point = pos2(150.0, 90.0);
    let mut driven = vec![Incoming::points(Input::PointerMoved(point)).in_window(WindowId::MAIN)];
    space.input(&mut driven, &view(now));
    let expected = screen(&space, main(&space).world(point));
    assert!(
        (space.cursor_screen().unwrap() - expected).length() < 0.01,
        "a position in a window's points puts it on that window's panel"
    );
    let got = delivered(&driven);
    let [(WindowId::MAIN, Input::PointerMoved(pos))] = got.as_slice() else {
        panic!("the move reaches the main window, not {got:?}");
    };
    assert!((*pos - point).length() < 0.01, "at that point: {pos:?}");

    let mut moved = vec![Incoming::points(Input::PointerMotion(vec2(10.0, 0.0)))];
    space.input(&mut moved, &view(now));
    assert!(
        (space.cursor_screen().unwrap() - (expected + vec2(10.0, 0.0))).length() < 0.01,
        "motion afterwards moves it from there"
    );
}
