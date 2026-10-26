use super::*;
use crate::{CURSOR_LIFT, OPEN_DISTANCE};

#[test]
fn a_miss_keeps_the_cursor_as_far_away_as_it_last_hit() {
    let (mut space, now) = space();
    let corner = pos2(2.0, 2.0);
    send(&mut space, now, [Input::PointerMoved(corner)]);
    assert!(
        (eye_distance(&space) - OPEN_DISTANCE * (1.0 - CURSOR_LIFT)).abs() < 1e-4,
        "with nothing hit yet the cursor is shown at the default distance"
    );

    send(&mut space, now, [Input::PointerMoved(centre())]);
    let on_panel = eye_distance(&space);
    send(&mut space, now, [Input::PointerMoved(corner)]);
    let at = space.cursor_at().expect("a miss still shows the cursor");
    assert!(
        (at.distance(space.head.position) - on_panel).abs() < 1e-4,
        "a miss keeps the distance of the last hit"
    );
    let ray = space.head.ray(PLATFORM, corner.to_vec2());
    assert!(
        (at - space.head.position).normalize().dot(ray.direction) > 0.9999,
        "the cursor stays on the pointer's ray"
    );

    send(&mut space, now, [Input::PointerLeft]);
    assert_eq!(
        space.cursor_at(),
        None,
        "a pointer that left the platform window shows no cursor"
    );
}
