use super::*;

#[test]
fn dragging_the_roll_control_with_the_head_rolled_rolls_the_panel_about_its_normal() {
    let (mut space, now) = space();
    roll_and_look_down(&mut space);
    let start = *main(&space);
    let grab = on_part(&space, WindowId::MAIN, Part::Roll);
    let to = grab + vec2(150.0, 0.0);
    send(
        &mut space,
        now,
        [
            Input::PointerMoved(grab),
            button(grab, true),
            Input::PointerMoved(to),
            button(to, false),
        ],
    );
    let panel = *main(&space);
    assert!(close(panel.pose.position, start.pose.position));
    assert!(
        close(panel.normal(), start.normal()),
        "it keeps facing the same way"
    );
    let (axis, angle) = (panel.pose.rotation * start.pose.rotation.inverse()).to_axis_angle();
    assert!(
        axis.dot(start.normal()) > 0.999 && angle > 0.1 && angle < std::f32::consts::PI,
        "dragged right along the screen below its centre, it turns counterclockwise to the eye, as with the head level: {axis:?} {angle}"
    );
}
