use super::*;

#[test]
fn squeezing_a_panel_with_both_hands_scales_and_turns_it() {
    let (mut space, now) = space();
    let panel = *main(&space);
    drive(
        &mut space,
        now,
        [
            hand_pose_of(Hand::Left, left_hand(), panel.world(pos2(200.0, 300.0))),
            hand_pose_of(Hand::Right, right_hand(), panel.world(pos2(600.0, 300.0))),
        ],
    );
    drive(&mut space, now, [squeeze(Hand::Left, 1.0)]);
    drive(&mut space, now, [squeeze(Hand::Right, 1.0)]);
    assert_eq!(space.both.map(|both| both.window), Some(WindowId::MAIN));
    let start = *main(&space);

    let (left, right) = (left_hand(), right_hand());
    let middle = (left + right) / 2.0;
    let turned = |at: Vec3| middle + glam::Quat::from_rotation_y(0.3) * (at - middle) * 1.5;
    let (left, right) = (turned(left), turned(right));
    let forward = |hand: Vec3| hand + Vec3::NEG_Z;
    drive(
        &mut space,
        later(now, 0.1),
        [
            hand_pose_of(Hand::Left, left, forward(left)),
            hand_pose_of(Hand::Right, right, forward(right)),
        ],
    );
    let after = *main(&space);
    assert!(
        (after.metres_per_point / start.metres_per_point - 1.5).abs() < 1e-3,
        "spreading the hands half as far again scales it half as much again"
    );
    assert!(
        after
            .pose
            .rotation
            .angle_between(glam::Quat::from_rotation_y(0.3) * start.pose.rotation)
            < 1e-3,
        "and turning them turns it"
    );
    assert_eq!(after.size, start.size, "the window keeps its points");

    let released = drive(&mut space, now, [squeeze(Hand::Left, 0.0)]);
    assert_eq!(placed(&released).len(), 1);
    assert_eq!(space.both, None);
    assert!(space.lasers.iter().all(|laser| laser.held.carry.is_none()));
}
