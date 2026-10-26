use super::*;

#[test]
fn a_miss_while_a_button_is_held_leaves_the_window_and_keeps_the_drag() {
    let (mut space, now) = space();
    let window = open_ahead(&mut space, now, 0.5);
    send(
        &mut space,
        now,
        [Input::PointerMoved(centre()), button(centre(), true)],
    );

    let corner = pos2(2.0, 2.0);
    let got = delivered(&send(&mut space, now, [Input::PointerMoved(corner)]));
    let [(left, Input::PointerLeft), (to, Input::PointerMoved(pos))] = got.as_slice() else {
        panic!("leaving every panel with a button held is PointerLeft then the move, not {got:?}");
    };
    assert_eq!((*left, *to), (window, window));
    assert!(
        pos.x < 0.0 && pos.y < 0.0,
        "the move is past the window's corner, in its plane: {pos:?}"
    );

    let released = delivered(&send(&mut space, now, [button(corner, false)]));
    let [
        (
            to,
            Input::PointerButton {
                pos,
                pressed: false,
                ..
            },
        ),
    ] = released.as_slice()
    else {
        panic!("the release still reaches the app, not {released:?}");
    };
    assert_eq!(*to, window);
    assert!(
        !Rect::from_min_size(Pos2::ZERO, SMALL).contains(*pos),
        "with no window under the pointer: {pos:?}"
    );

    assert_eq!(
        delivered(&send(&mut space, now, [Input::PointerMoved(corner)])),
        [],
        "once let go, a miss reaches nothing"
    );
}
