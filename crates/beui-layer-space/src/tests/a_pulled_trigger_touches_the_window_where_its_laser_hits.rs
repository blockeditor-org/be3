use super::*;

#[test]
fn a_pulled_trigger_touches_the_window_where_its_laser_hits() {
    let (mut space, now) = space();
    let window = open_ahead(&mut space, now, 0.4);
    assert_eq!(space.focused(), Some(window));
    let at = pos2(200.0, 150.0);
    let target = main(&space).world(at);

    let aimed = drive(&mut space, now, [hand_pose(right_hand(), target)]);
    assert_eq!(touches(&aimed), [], "a laser does not hover");
    let pressed = drive(&mut space, now, [trigger(Hand::Right, 0.7)]);
    let got = touches(&pressed);
    assert_eq!(got.len(), 1, "{got:?}");
    let (on, finger, phase, pos) = got[0];
    assert_eq!((on, finger, phase), (WindowId::MAIN, 1, TouchPhase::Start));
    assert!(
        near(pos, at),
        "the touch lands where the laser hits: {pos:?}"
    );
    assert_eq!(
        space.focused(),
        Some(WindowId::MAIN),
        "and focuses its panel"
    );
    assert_eq!(
        drive(&mut space, now, [trigger(Hand::Right, 0.5)]),
        [],
        "the trigger lets go only below where it pressed"
    );
    let released = touches(&drive(&mut space, now, [trigger(Hand::Right, 0.1)]));
    assert_eq!(released.len(), 1);
    assert_eq!(released[0].2, TouchPhase::End);
    assert!(near(released[0].3, at));

    let small = space.panel(window).unwrap().world(pos2(100.0, 75.0));
    drive(
        &mut space,
        now,
        [hand_pose_of(Hand::Left, left_hand(), small)],
    );
    let other = touches(&drive(&mut space, now, [trigger(Hand::Left, 1.0)]));
    assert_eq!(other.len(), 1);
    assert_eq!(
        (other[0].0, other[0].1),
        (window, 0),
        "each hand is its own finger"
    );
    assert_eq!(space.focused(), Some(window));
}
