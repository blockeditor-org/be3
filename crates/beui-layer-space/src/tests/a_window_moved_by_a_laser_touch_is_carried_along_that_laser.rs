use super::*;

#[test]
fn a_window_moved_by_a_laser_touch_is_carried_along_that_laser() {
    let (mut space, now) = space();
    let window = open_ahead(&mut space, now, 0.6);
    let bar = pos2(100.0, 10.0);
    let grab = space.panel(window).unwrap().world(bar);
    drive(
        &mut space,
        now,
        [hand_pose_of(Hand::Left, left_hand(), grab)],
    );
    drive(&mut space, now, [trigger(Hand::Left, 1.0)]);
    space.request(&WindowRequest::BeginMove(window));
    assert!(space.mouse.carry.is_none(), "the mouse did not ask");
    assert_eq!(
        space
            .laser(Hand::Left)
            .and_then(|laser| laser.held.carry.as_ref())
            .map(|carry| carry.window),
        Some(window)
    );
    let distance = grab.distance(left_hand());
    let to = left_hand() + Vec3::new(-0.5, 0.2, -1.0).normalize() * distance;
    let moved = drive(
        &mut space,
        later(now, 0.5),
        [hand_pose_of(Hand::Left, left_hand(), to)],
    );
    assert_eq!(
        touches(&moved),
        [],
        "the app sees no moves while it is carried"
    );
    let held = space.panel(window).unwrap().world(bar);
    assert!(off_ray(held, left_hand(), to) < 0.005, "{held} {to}");

    let released = drive(&mut space, later(now, 0.5), [trigger(Hand::Left, 0.0)]);
    let ended = touches(&released);
    assert_eq!(ended.len(), 1);
    assert_eq!((ended[0].0, ended[0].2), (window, TouchPhase::End));
    assert!(
        near(ended[0].3, bar),
        "it lets go where it grabbed: {:?}",
        ended[0].3
    );
    assert_eq!(placed(&released).len(), 1);
}
