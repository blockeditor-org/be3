use super::the_chrome_around_a_curved_panel::window;
use super::*;

#[test]
fn two_lasers_pinching_a_panel() {
    let (mut space, now, context) = painted_space();
    let panel = *main(&space);
    drive(
        &mut space,
        now,
        [
            hand_pose_of(Hand::Left, left_hand(), panel.world(pos2(340.0, 320.0))),
            hand_pose_of(Hand::Right, right_hand(), panel.world(pos2(460.0, 280.0))),
            trigger(Hand::Left, 1.0),
            trigger(Hand::Right, 1.0),
        ],
    );
    let spread = drive(
        &mut space,
        later(now, 0.5),
        [
            hand_pose_of(Hand::Left, left_hand(), panel.world(pos2(200.0, 380.0))),
            hand_pose_of(Hand::Right, right_hand(), panel.world(pos2(600.0, 220.0))),
        ],
    );
    assert_eq!(
        touches(&spread)
            .iter()
            .filter(|touch| touch.2 == TouchPhase::Move)
            .count(),
        2
    );
    assert_scene(
        &mut space,
        now,
        &[window(&context, WindowId::MAIN, PLATFORM, "Pinched")],
        "two_lasers_pinching_a_panel",
    );
}
