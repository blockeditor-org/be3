use super::*;

#[test]
fn keys_a_driver_sends_steer_too() {
    let (mut space, now) = space();
    let mut incoming: Vec<Incoming> = steer(true).into_iter().map(Incoming::points).collect();
    space.input(&mut incoming, &view(now));
    assert!(space.steering(), "drive's Ctrl and Shift steer");
    let mut pressed = vec![Incoming::points(key(Key::W, 17, true))];
    space.input(&mut pressed, &view(now));
    assert_eq!(delivered(&pressed), [], "drive's W walks");
    let start = space.head.position;
    send(&mut space, later(now, 0.5), []);
    assert!(space.head.position.z < start.z, "forward");
}
