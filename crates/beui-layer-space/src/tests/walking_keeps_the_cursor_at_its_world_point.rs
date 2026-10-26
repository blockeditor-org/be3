use super::*;

#[test]
fn walking_keeps_the_cursor_at_its_world_point() {
    let (mut space, now) = space();
    let at = centre() + vec2(100.0, 50.0);
    send(&mut space, now, [Input::PointerMoved(at)]);
    let saved = space.cursor().expect("the cursor is placed");

    send(&mut space, now, steer(true));
    send(&mut space, now, [key(Key::A, 30, true)]);
    send(&mut space, later(now, 0.1), []);
    send(&mut space, later(now, 0.2), [key(Key::A, 30, false)]);
    send(&mut space, later(now, 0.2), steer(false));
    assert!(space.head.position.x < -0.1, "the head walked left");
    assert_eq!(space.cursor(), Some(saved), "the cursor stays where it was");
    assert_eq!(space.cursor_at(), Some(saved));
    assert!(
        space.cursor_screen().unwrap().x > at.x + 1.0,
        "so it moves right across the screen"
    );
}
