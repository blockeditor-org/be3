use super::*;

#[test]
fn a_miss_leaves_the_panel() {
    let (mut space, now) = space();
    send(&mut space, now, [Input::PointerMoved(centre())]);
    let corner = pos2(2.0, 2.0);

    assert_eq!(
        delivered(&send(&mut space, now, [Input::PointerMoved(corner)])),
        [(WindowId::MAIN, Input::PointerLeft)],
        "moving off the panel leaves it"
    );
    assert_eq!(
        delivered(&send(
            &mut space,
            now,
            [
                Input::PointerMoved(corner),
                button(corner, true),
                button(corner, false),
                scroll(1.0),
            ]
        )),
        [],
        "nothing that misses reaches the app"
    );
}
