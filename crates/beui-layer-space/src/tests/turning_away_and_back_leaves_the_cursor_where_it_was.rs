use super::*;

#[test]
fn turning_away_and_back_leaves_the_cursor_where_it_was() {
    let (mut space, now) = space();
    let at = centre() + vec2(60.0, -40.0);
    send(&mut space, now, [Input::PointerMoved(at)]);
    let saved = space.cursor().expect("the cursor is placed");

    send(&mut space, now, steer(true));
    send(&mut space, now, [Input::PointerMotion(vec2(80.0, 0.0))]);
    assert_eq!(space.cursor(), Some(saved), "turning keeps its world point");
    assert_eq!(space.cursor_at(), Some(saved), "and draws it there");
    let slid = space.cursor_screen().unwrap();
    assert!(
        slid.x < at.x - 50.0 && (slid.y - at.y).abs() < 1.0,
        "turning right slides it left across the screen: {slid:?}"
    );

    send(&mut space, now, [Input::PointerMotion(vec2(-80.0, 0.0))]);
    send(&mut space, now, steer(false));
    assert_eq!(space.cursor(), Some(saved));
    assert!(
        (space.cursor_screen().unwrap() - at).length() < 0.01,
        "turning back finds it where it was"
    );
}
