use super::*;

#[test]
fn a_released_panel_turns_to_face_the_viewer() {
    let (mut space, now) = space();
    send(&mut space, now, [Input::PointerMoved(centre())]);
    space.begin_carry(WindowId::MAIN);
    send(&mut space, now, steer(true));
    send(&mut space, now, [Input::PointerMotion(vec2(300.0, -150.0))]);
    send(&mut space, now, steer(false));
    let tilted = main(&space).normal();
    assert!(
        tilted.y.abs() > 0.1,
        "carried while looking up, it tilts with the head"
    );

    let released = send(
        &mut space,
        now,
        [button(centre(), true), button(centre(), false)],
    );
    assert_eq!(
        space.carry, None,
        "a carry begun without a button ends at the next click"
    );
    assert_eq!(
        delivered(&released),
        [],
        "that click is the carry's, not the app's"
    );
    assert_eq!(placed(&released).len(), 1);

    let panel = main(&space);
    let normal = panel.normal();
    assert!(normal.y.abs() < 1e-4, "it stands upright: {normal:?}");
    let toward = space.head.position - panel.pose.position;
    let level = Vec3::new(toward.x, 0.0, toward.z).normalize();
    assert!(
        close(normal, level),
        "it faces the viewer: {normal:?} {level:?}"
    );
}
