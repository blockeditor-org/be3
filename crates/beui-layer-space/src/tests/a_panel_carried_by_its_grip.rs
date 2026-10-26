use super::the_chrome_around_a_curved_panel::window;
use super::*;
use crate::carry::TURN_SECONDS;

#[test]
fn a_panel_carried_by_its_grip() {
    let (mut space, now, context) = painted_space();
    let popped = open(
        &mut space,
        now,
        WindowPlacement::Over {
            window: WindowId::MAIN,
            at: pos2(40.0, 60.0),
        },
    )
    .0;
    let grab = space.panel(popped).unwrap().world(pos2(100.0, 75.0));
    drive(
        &mut space,
        now,
        [hand_pose(right_hand(), grab), squeeze(Hand::Right, 1.0)],
    );
    let distance = grab.distance(right_hand());
    let to = right_hand() + (Vec3::new(0.25, 1.75, -0.62) - right_hand()).normalize() * distance;
    let now = later(now, TURN_SECONDS);
    drive(&mut space, now, [hand_pose(right_hand(), to)]);
    assert!(
        off_ray(
            space.panel(popped).unwrap().world(pos2(100.0, 75.0)),
            right_hand(),
            to
        ) < 0.005
    );
    let windows = [
        window(&context, WindowId::MAIN, PLATFORM, "Main"),
        window(&context, popped, SMALL, "Carried"),
    ];
    assert_scene(&mut space, now, &windows, "a_panel_carried_by_its_grip");
}
