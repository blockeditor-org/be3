use super::*;
use crate::CURSOR_MARGIN;

#[test]
fn a_cursor_turned_out_of_view_is_drawn_inside_the_edge_and_clicks_there() {
    let (mut space, now) = space();
    let at = centre() + vec2(-200.0, 0.0);
    send(&mut space, now, [Input::PointerMoved(at)]);
    let saved = space.cursor().expect("the cursor is placed");

    send(&mut space, now, steer(true));
    send(&mut space, now, [Input::PointerMotion(vec2(250.0, 0.0))]);
    let drawn = space.cursor_screen().unwrap();
    assert!(
        (drawn.x - CURSOR_MARGIN).abs() < 0.01,
        "turned right, it stops just inside the left edge: {drawn:?}"
    );
    assert_eq!(space.cursor(), Some(saved), "without moving its point");
    send(&mut space, now, steer(false));

    let got = delivered(&send(&mut space, now, [button(at, true)]));
    let [(window, input)] = got.as_slice() else {
        panic!("the press reaches the app once, not {got:?}");
    };
    assert_eq!(*window, WindowId::MAIN);
    assert!(
        (pos_of(input) - on(&space, WindowId::MAIN, drawn)).length() < 0.01,
        "where the cursor is drawn, not where it was saved: {input:?}"
    );
    assert_eq!(space.cursor(), Some(saved), "a click saves no new point");

    send(&mut space, now, steer(true));
    send(&mut space, now, [Input::PointerMotion(vec2(-250.0, 0.0))]);
    assert!(
        (space.cursor_screen().unwrap() - at).length() < 0.01,
        "turning back finds it where it was"
    );
}
