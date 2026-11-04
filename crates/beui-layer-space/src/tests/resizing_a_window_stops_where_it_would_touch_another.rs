use glam::Quat;

use super::*;
use crate::contact;

#[test]
fn resizing_a_window_stops_where_it_would_touch_another() {
    let (mut space, now) = space();
    let turned = Pose {
        position: Head::default().position + Vec3::new(0.05, 0.0, -0.55),
        rotation: Quat::from_rotation_y(0.5),
    };
    let (window, _) = open(
        &mut space,
        now,
        WindowPlacement::Host(panel(WindowId::MAIN, turned).placement()),
    );
    let start = *space.panel(window).unwrap();
    assert!(!contact::touching(
        &start,
        main(&space),
        space.head.position
    ));
    let grip = screen(&space, start.world(pos2(SMALL.x - 4.0, SMALL.y / 2.0)));
    send(
        &mut space,
        now,
        [Input::PointerMoved(grip), button(grip, true)],
    );
    space.request(&WindowRequest::BeginResize {
        window,
        edge: WindowEdge::Right,
    });
    for step in 1..=20 {
        let to = grip + vec2(15.0 * step as f32, 0.0);
        send(&mut space, now, [Input::PointerMoved(to)]);
        assert!(
            !contact::touching(
                space.panel(window).unwrap(),
                main(&space),
                space.head.position
            ),
            "step {step}"
        );
    }
    let grown = space.panel(window).unwrap().size.x;
    assert!(
        grown > SMALL.x + 10.0,
        "it grows until it would touch: {grown}"
    );
    let free = start
        .intersect(
            space
                .head
                .ray(PLATFORM, (grip + vec2(300.0, 0.0)).to_vec2()),
        )
        .unwrap()
        .pos
        .x;
    assert!(grown < free - 10.0, "and stops there: {grown} {free}");
}
