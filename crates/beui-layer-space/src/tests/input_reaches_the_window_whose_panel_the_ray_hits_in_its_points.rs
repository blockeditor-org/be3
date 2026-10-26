use super::*;

#[test]
fn input_reaches_the_window_whose_panel_the_ray_hits_in_its_points() {
    let (mut space, now) = space();
    let window = open_ahead(&mut space, now, 0.5);
    let inside = centre() + vec2(50.0, 20.0);
    let expected = on(&space, window, inside);
    assert!((expected.x - SMALL.x / 2.0) > 10.0, "{expected:?}");

    let got = delivered(&send(
        &mut space,
        now,
        [Input::PointerMoved(inside), button(inside, true)],
    ));
    assert_eq!(got.len(), 2);
    for (to, input) in &got {
        assert_eq!(*to, window, "the nearer panel takes the pointer");
        assert!((pos_of(input) - expected).length() < 0.01, "{input:?}");
    }

    let beside = centre() + vec2(250.0, 0.0);
    let behind = on(&space, WindowId::MAIN, beside);
    let dragged = delivered(&send(&mut space, now, [Input::PointerMoved(beside)]));
    let [(to, moved)] = dragged.as_slice() else {
        panic!("a drag into another window is one move there, not {dragged:?}");
    };
    assert_eq!(
        *to,
        WindowId::MAIN,
        "while held, the window under the pointer is named"
    );
    assert!((pos_of(moved) - behind).length() < 0.01);

    let released = delivered(&send(&mut space, now, [button(beside, false)]));
    assert_eq!(
        released.iter().map(|(to, _)| *to).collect::<Vec<_>>(),
        [WindowId::MAIN],
        "the release is where the pointer is"
    );
}
