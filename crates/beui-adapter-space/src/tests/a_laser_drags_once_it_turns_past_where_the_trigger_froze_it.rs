use super::*;
use crate::laser::FREEZE;

#[test]
fn a_laser_drags_once_it_turns_past_where_the_trigger_froze_it() {
    let (mut space, now) = space();
    let at = pos2(400.0, 300.0);
    let target = main(&space).world(at);
    drive(&mut space, now, [hand_pose(right_hand(), target)]);
    drive(&mut space, now, [trigger(Hand::Right, 1.0)]);
    let distance = target.distance(right_hand());

    let flinch = target + Vec3::X * distance * FREEZE * 0.5;
    let now = later(now, 0.02);
    let flinched = drive(&mut space, now, [hand_pose(right_hand(), flinch)]);
    assert_eq!(
        touches(&flinched),
        [],
        "the jolt of pulling the trigger moves nothing"
    );

    let mut now = now;
    let mut moves = Vec::new();
    for step in 1..=20 {
        now = later(now, 0.02);
        let along = target + Vec3::X * 0.004 * step as f32;
        moves.extend(touches(&drive(
            &mut space,
            now,
            [hand_pose(right_hand(), along)],
        )));
    }
    assert!(
        moves
            .iter()
            .all(|(window, _, phase, _)| (*window, *phase) == (WindowId::MAIN, TouchPhase::Move)),
        "{moves:?}"
    );
    let last = moves.last().expect("turning farther drags the touch").3;
    assert!(
        last.x > at.x + 20.0,
        "the touch follows the laser: {last:?}"
    );
    let released = touches(&drive(&mut space, now, [trigger(Hand::Right, 0.0)]));
    assert_eq!(released.len(), 1);
    assert_eq!(released[0].2, TouchPhase::End);
}
