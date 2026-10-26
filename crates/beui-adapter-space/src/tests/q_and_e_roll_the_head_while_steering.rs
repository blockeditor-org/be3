use super::*;
use crate::head::ROLL_PER_SECOND;

#[test]
fn q_and_e_roll_the_head_while_steering() {
    let (mut space, now) = space();
    send(&mut space, now, steer(true));
    let pressed = send(&mut space, now, [key(Key::Q, 16, true)]);
    assert_eq!(
        delivered(&pressed),
        [],
        "the app does not get Q while steering"
    );
    for step in 1..=10 {
        send(&mut space, later(now, 0.05 * step as f32), []);
    }
    assert!(
        (space.head.roll - 0.5 * ROLL_PER_SECOND).abs() < 1e-4,
        "holding Q rolls the head left: {}",
        space.head.roll
    );
    let released = send(&mut space, later(now, 0.5), [key(Key::Q, 16, false)]);
    assert_eq!(delivered(&released), []);
    send(&mut space, later(now, 0.5), [key(Key::E, 18, true)]);
    for step in 11..=30 {
        send(&mut space, later(now, 0.05 * step as f32), []);
    }
    send(&mut space, later(now, 1.5), [key(Key::E, 18, false)]);
    assert!(
        (space.head.roll + 0.5 * ROLL_PER_SECOND).abs() < 1e-4,
        "holding E rolls it right: {}",
        space.head.roll
    );
    let right = space.head.rotation() * Vec3::X;
    assert!(right.y < -0.1, "its right side goes down: {right:?}");
}
