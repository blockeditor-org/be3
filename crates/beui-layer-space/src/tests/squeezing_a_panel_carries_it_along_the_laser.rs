use super::*;
use crate::carry::TURN_SECONDS;

#[test]
fn squeezing_a_panel_carries_it_along_the_laser() {
    let (mut space, now) = space();
    let window = open_ahead(&mut space, now, 0.6);
    let grab = space.panel(window).unwrap().world(pos2(100.0, 75.0));
    drive(&mut space, now, [hand_pose(right_hand(), grab)]);
    let squeezed = drive(&mut space, now, [squeeze(Hand::Right, 1.0)]);
    assert_eq!(touches(&squeezed), [], "the grip reaches no app");
    assert_eq!(
        space
            .laser(Hand::Right)
            .and_then(|laser| laser.held.carry.as_ref())
            .map(|carry| carry.window),
        Some(window)
    );
    assert_eq!(space.focused(), Some(window));

    let distance = grab.distance(right_hand());
    let to = right_hand() + (Vec3::new(0.6, 0.4, -1.0)).normalize() * distance;
    let now = later(now, TURN_SECONDS);
    drive(&mut space, now, [hand_pose(right_hand(), to)]);
    let carried = space.panel(window).unwrap();
    let held = carried.world(pos2(100.0, 75.0));
    assert!(
        off_ray(held, right_hand(), to) < 0.005,
        "the grabbed point rides the laser: {held} {to}"
    );
    assert!(
        facing_eye(&space, carried) < 0.05,
        "and it turns to face the viewer"
    );

    let released = drive(&mut space, now, [squeeze(Hand::Right, 0.0)]);
    assert_eq!(placed(&released).len(), 1);
    assert!(space.laser(Hand::Right).unwrap().held.carry.is_none());
    assert_eq!(space.mouse.carry, None, "the mouse holds nothing all along");
}
