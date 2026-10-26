use super::*;
use crate::laser::JUMP;

#[test]
fn a_laser_is_smoothed_and_takes_a_jump_at_once() {
    let (mut space, now) = space();
    let ahead = right_hand() + Vec3::NEG_Z;
    drive(&mut space, now, [hand_pose(right_hand(), ahead)]);
    let aimed = |space: &Space| space.laser(Hand::Right).unwrap().ray.unwrap().direction;
    assert!(aimed(&space).angle_between(Vec3::NEG_Z) < 1e-4);

    let little = right_hand() + glam::Quat::from_rotation_y(JUMP / 2.0) * Vec3::NEG_Z;
    let now = later(now, 0.011);
    drive(&mut space, now, [hand_pose(right_hand(), little)]);
    let turned = aimed(&space).angle_between(Vec3::NEG_Z);
    assert!(
        turned > 0.0 && turned < JUMP / 2.0 * 0.5,
        "a small turn is followed partly at first: {turned}"
    );
    let mut now = now;
    for _ in 0..20 {
        now = later(now, 0.1);
        drive(&mut space, now, []);
    }
    let settled = aimed(&space).angle_between(Vec3::NEG_Z);
    assert!(
        (settled - JUMP / 2.0).abs() < 1e-3,
        "and wholly once it holds still: {settled}"
    );

    let far = right_hand() + glam::Quat::from_rotation_y(-1.0) * Vec3::NEG_Z;
    drive(
        &mut space,
        later(now, 0.011),
        [hand_pose(right_hand(), far)],
    );
    let jumped = aimed(&space).angle_between(glam::Quat::from_rotation_y(-1.0) * Vec3::NEG_Z);
    assert!(jumped < 1e-4, "a jump is taken at once: {jumped}");
}
