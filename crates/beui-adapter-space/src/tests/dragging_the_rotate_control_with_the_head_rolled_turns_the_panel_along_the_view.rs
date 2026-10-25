use super::*;

fn turn(space: &Space, start: &Panel) -> (Vec3, f32) {
    let (axis, angle) = (main(space).pose.rotation * start.pose.rotation.inverse()).to_axis_angle();
    match angle > std::f32::consts::PI {
        true => (-axis, std::f32::consts::TAU - angle),
        false => (axis, angle),
    }
}

#[test]
fn dragging_the_rotate_control_with_the_head_rolled_turns_the_panel_along_the_view() {
    let (mut space, now) = space();
    roll_and_look_down(&mut space);
    let start = *main(&space);
    let (up, right) = (space.head.up(), space.head.rotation() * Vec3::X);
    let grab = on_part(&space, WindowId::MAIN, Part::Turn);
    let to = grab + vec2(80.0, 0.0);
    send(
        &mut space,
        now,
        [
            Input::PointerMoved(grab),
            button(grab, true),
            Input::PointerMoved(to),
        ],
    );
    let (axis, angle) = turn(&space, &start);
    assert!(angle > 0.05, "dragging along the screen turns it: {angle}");
    assert!(
        axis.dot(up) > 0.999,
        "about the head's up, not the world's: {axis:?} {up:?}"
    );
    assert!(
        main(&space).normal().dot(right) > 0.05,
        "to the screen's right"
    );
    assert!(close(main(&space).pose.position, start.pose.position));

    let down = grab + vec2(0.0, 60.0);
    send(&mut space, now, [Input::PointerMoved(down)]);
    let (axis, angle) = turn(&space, &start);
    assert!(angle > 0.05, "dragging down the screen tilts it: {angle}");
    assert!(
        axis.dot(right) > 0.999,
        "about the head's right: {axis:?} {right:?}"
    );
    assert!(
        main(&space).normal().dot(up) < -0.05,
        "tipping its top toward the viewer"
    );
    send(&mut space, now, [button(down, false)]);
}
