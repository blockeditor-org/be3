use super::*;
use crate::carry::TURN_SECONDS;

#[test]
fn a_panel_carried_with_the_head_rolled_rolls_to_match() {
    let (mut space, now) = space();
    space.head.roll = 0.4;
    send(
        &mut space,
        now,
        [Input::PointerMoved(centre()), button(centre(), true)],
    );
    space.request(&WindowRequest::BeginMove(WindowId::MAIN));
    send(&mut space, now, []);
    send(
        &mut space,
        later(now, TURN_SECONDS),
        [button(centre(), false)],
    );
    assert_eq!(space.carry, None);
    let panel = main(&space);
    let across = panel.pose.rotation * Vec3::X;
    let right = space.head.rotation() * Vec3::X;
    assert!(
        across.angle_between(right) < 1e-3,
        "its edges line up with the view: {across:?} {right:?}"
    );
    assert!(facing_eye(&space, panel) < 1e-3, "and it faces the viewer");
}
