use super::*;

#[test]
fn a_carried_panel_swings_about_the_view_s_up_with_the_head_rolled() {
    let (mut space, now) = space();
    space.head.roll = std::f32::consts::FRAC_PI_2;
    send(&mut space, now, [Input::PointerMoved(centre())]);
    space.begin_carry(Pointer::Mouse, WindowId::MAIN);
    let in_view = |space: &Space| space.head.rotation().inverse() * main(space).pose.rotation;
    let before = in_view(&space);
    send(
        &mut space,
        now,
        [Input::PointerMoved(centre() + vec2(200.0, 0.0))],
    );
    let (axis, angle) = (in_view(&space) * before.inverse()).to_axis_angle();
    assert!(
        angle > 0.1 && axis.abs().dot(Vec3::Y) > 0.999,
        "moving the mouse along the rolled screen swings the panel about the view's up, \
         as the turn to face the viewer uses: {axis:?} by {angle}"
    );
}
