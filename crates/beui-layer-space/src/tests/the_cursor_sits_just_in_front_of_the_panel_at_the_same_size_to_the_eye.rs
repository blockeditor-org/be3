use super::*;
use crate::CURSOR_LIFT;

#[test]
fn the_cursor_sits_just_in_front_of_the_panel_at_the_same_size_to_the_eye() {
    let (mut space, now) = space();
    send(&mut space, now, [Input::PointerMoved(centre())]);
    let hit = main(&space).pose.position.distance(space.head.position);
    let far = eye_distance(&space);
    assert!(
        far < hit && far > hit * (1.0 - 2.0 * CURSOR_LIFT),
        "the cursor is a little in front of the panel: {far} for a panel {hit} away"
    );

    let near = open_ahead(&mut space, now, hit / 2.0);
    send(&mut space, now, [Input::PointerMoved(centre())]);
    assert!(space.panel(near).is_some());
    let close = eye_distance(&space);
    assert!(
        (close - hit / 2.0 * (1.0 - CURSOR_LIFT)).abs() < 1e-3,
        "the nearer panel takes the cursor"
    );

    let angle = |space: &Space, distance: f32| {
        let at = space.cursor_at().expect("the cursor is shown");
        space.cursor_scale(at) * crate::cursor::SIZE.y / distance
    };
    let pixels = |space: &Space, distance: f32| {
        angle(space, distance) / (2.0 * (crate::head::FOV_Y / 2.0).tan()) * PLATFORM.y
    };
    assert!(
        (pixels(&space, close) - crate::cursor::SIZE.y).abs() < 0.01,
        "the cursor covers as many pixels as it has points"
    );
    space.close(near);
    send(&mut space, now, [Input::PointerMoved(centre())]);
    assert!((eye_distance(&space) - far).abs() < 1e-3);
    assert!(
        (pixels(&space, far) - crate::cursor::SIZE.y).abs() < 0.01,
        "twice as far it is twice as big, so it looks the same size"
    );
}
