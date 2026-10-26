use super::*;
use crate::head::{Fov, View, Views};

#[test]
fn steering_with_a_headset_moves_the_body_the_views_are_tracked_in() {
    let (mut space, now) = space();
    let eyes = Head::default().position;
    let tracked = |at: Instant| Views {
        display_time: at,
        views: vec![View {
            position: eyes,
            rotation: glam::Quat::IDENTITY,
            fov: Fov::from_angles(-0.8, 0.8, 0.8, -0.8),
            size: PLATFORM,
        }],
    };
    space.set_views(Some(tracked(now)));
    send(&mut space, now, steer(true));
    assert!(space.steering());

    send(&mut space, now, [Input::PointerMotion(vec2(200.0, -300.0))]);
    let turn = 200.0 * crate::head::TURN_PER_PIXEL;
    assert!(
        (space.head.yaw - turn).abs() < 1e-4 && space.head.pitch.abs() < 1e-4,
        "moving the mouse sideways turns the body under the headset, and up does nothing: \
         yaw {} pitch {}",
        space.head.yaw,
        space.head.pitch
    );
    assert!(
        close(space.head.position, eyes),
        "the body turns about the eyes"
    );

    send(&mut space, now, [key(Key::W, 17, true)]);
    let mut at = now;
    for _ in 0..5 {
        at = later(at, 0.1);
        space.set_views(Some(tracked(at)));
        send(&mut space, at, []);
    }
    space.set_views(Some(tracked(at)));
    let walked = space.head.position - eyes;
    let forward = glam::Quat::from_rotation_y(-turn) * Vec3::NEG_Z;
    assert!(
        walked.length() > 0.5 && walked.normalize().distance(forward) < 1e-3,
        "W walks the way the body now faces, and the same tracked pose stays walked: {walked:?}"
    );
    assert!(
        (space.head.yaw - turn).abs() < 1e-4,
        "and stays turned: {}",
        space.head.yaw
    );
    drive(&mut space, at, [hand_pose(eyes, eyes + Vec3::NEG_Z)]);
    let aim = space
        .laser(Hand::Right)
        .and_then(|laser| laser.aim)
        .expect("the hand is tracked");
    assert!(
        close(aim.position, space.head.position)
            && (aim.rotation * Vec3::NEG_Z).distance(forward) < 1e-3,
        "a hand tracked where the eyes are is placed with them, by the body: {aim:?}"
    );
    send(&mut space, at, [key(Key::W, 17, false)]);
    send(&mut space, at, steer(false));

    space.set_views(None);
    let left = space.head;
    space.set_views(Some(tracked(at)));
    assert_eq!(
        space.head, left,
        "the body is kept while the headset is away"
    );
}
