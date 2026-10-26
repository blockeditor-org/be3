use super::*;

#[test]
fn motion_a_driver_sends_turns_the_head_while_steering() {
    let motion = Input::PointerMotion(vec2(120.0, -30.0));
    let (mut driven, now) = space();
    let mut free = vec![Incoming::points(motion.clone())];
    driven.input(&mut free, &view(now));
    assert_eq!(
        driven.head,
        Head::default(),
        "motion without steering leaves the head"
    );

    let mut held: Vec<Incoming> = steer(true).into_iter().map(Incoming::points).collect();
    driven.input(&mut held, &view(now));
    let mut moved = vec![Incoming::points(motion.clone())];
    driven.input(&mut moved, &view(now));
    assert_eq!(delivered(&moved), [], "the app does not see the motion");

    let (mut platform, now) = space();
    send(&mut platform, now, steer(true));
    send(&mut platform, now, [motion]);
    assert!(driven.head.yaw > 0.0, "the head turns right");
    assert_eq!(
        driven.head, platform.head,
        "as the platform's motion turns it"
    );
}
