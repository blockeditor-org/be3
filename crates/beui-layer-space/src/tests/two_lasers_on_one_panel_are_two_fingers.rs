use super::*;

#[test]
fn two_lasers_on_one_panel_are_two_fingers() {
    let (mut space, now) = space();
    let (left_at, right_at) = (pos2(300.0, 300.0), pos2(500.0, 300.0));
    let panel = *main(&space);
    drive(
        &mut space,
        now,
        [
            hand_pose_of(Hand::Left, left_hand(), panel.world(left_at)),
            hand_pose_of(Hand::Right, right_hand(), panel.world(right_at)),
        ],
    );
    let down = touches(&drive(
        &mut space,
        now,
        [trigger(Hand::Left, 1.0), trigger(Hand::Right, 1.0)],
    ));
    assert_eq!(down.len(), 2);
    assert!(down.iter().all(|touch| touch.2 == TouchPhase::Start));
    assert_ne!(down[0].1, down[1].1);

    let spread = drive(
        &mut space,
        later(now, 0.1),
        [
            hand_pose_of(Hand::Left, left_hand(), panel.world(pos2(150.0, 300.0))),
            hand_pose_of(Hand::Right, right_hand(), panel.world(pos2(650.0, 300.0))),
        ],
    );
    let moved = touches(&spread);
    let x = |finger: u64| {
        moved
            .iter()
            .rev()
            .find(|touch| touch.1 == finger && touch.2 == TouchPhase::Move)
            .map(|touch| touch.3.x)
            .expect("both fingers move")
    };
    assert!(
        x(0) < 160.0 && x(1) > 640.0,
        "they spread apart as a pinch: {moved:?}"
    );
}
