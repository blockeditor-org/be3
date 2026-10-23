use super::*;
use beui_core::input::Modifiers;

#[test]
fn holding_right_ctrl_and_right_shift_steers_the_head() {
    let (mut space, now) = space();

    let left = delivered(&send(
        &mut space,
        now,
        [key(Key::Ctrl, 29, true), key(Key::Shift, 42, true)],
    ));
    assert_eq!(left.len(), 2, "left Ctrl and Shift reach the app");
    assert!(!space.steering(), "and do not steer");
    send(
        &mut space,
        now,
        [key(Key::Ctrl, 29, false), key(Key::Shift, 42, false)],
    );

    let got = delivered(&send(&mut space, now, steer(true)));
    assert!(space.steering());
    assert_eq!(
        got,
        [
            (WindowId::MAIN, key(Key::Ctrl, RIGHT_CTRL, true)),
            (WindowId::MAIN, key(Key::Ctrl, RIGHT_CTRL, false)),
            (WindowId::MAIN, Input::Modifiers(Modifiers::NONE)),
        ],
        "right Ctrl alone is the app's, until Shift joins it and the app sees it let go"
    );

    let yaw = space.head.yaw;
    let turned = send(&mut space, now, [Input::PointerMotion(vec2(100.0, -40.0))]);
    assert_eq!(delivered(&turned), [], "motion turns the head, not the app");
    assert!(space.head.yaw > yaw, "moving right turns right");
    assert!(space.head.pitch > 0.0, "moving up looks up");
    send(
        &mut space,
        now,
        [Input::PointerMotion(vec2(0.0, -100_000.0))],
    );
    assert!(
        space.head.pitch <= crate::head::MAX_PITCH,
        "the head looks no further than straight up"
    );
    send(
        &mut space,
        now,
        [Input::PointerMotion(vec2(-100.0, 100_000.0))],
    );
    space.head.pitch = 0.0;
    space.head.yaw = 0.0;

    let start = space.head.position;
    let pressed = send(&mut space, now, [key(Key::W, 17, true)]);
    assert_eq!(
        delivered(&pressed),
        [],
        "W walks and does not reach the app"
    );
    let mut at = now;
    for _ in 0..5 {
        at = later(at, 0.1);
        send(&mut space, at, []);
    }
    let walked = space.head.position - start;
    assert!(
        walked.z < -0.5 && walked.x.abs() < 1e-4 && walked.y.abs() < 1e-4,
        "W walks forward, along the floor: {walked:?}"
    );

    space.head.yaw = std::f32::consts::FRAC_PI_2;
    let before = space.head.position;
    send(&mut space, later(at, 0.1), []);
    let walked = space.head.position - before;
    assert!(
        walked.x > 0.1 && walked.z.abs() < 1e-4,
        "forward follows where the head faces: {walked:?}"
    );

    let released = send(&mut space, later(at, 0.2), steer(false));
    assert!(!space.steering(), "letting go stops steering");
    assert!(
        delivered(&released)
            .iter()
            .all(|(_, input)| matches!(input, Input::Modifiers(_))),
        "the modifiers' release does not reach the app as keys"
    );
    assert_eq!(
        delivered(&send(&mut space, later(at, 0.3), [key(Key::W, 17, false)])),
        [],
        "the release of a key that walked is swallowed too"
    );
    let position = space.head.position;
    send(&mut space, later(at, 0.4), []);
    assert_eq!(space.head.position, position, "walking stops");
    assert_eq!(
        delivered(&send(&mut space, later(at, 0.5), [key(Key::W, 17, true)])),
        [(WindowId::MAIN, key(Key::W, 17, true))],
        "afterwards W types again"
    );
}
