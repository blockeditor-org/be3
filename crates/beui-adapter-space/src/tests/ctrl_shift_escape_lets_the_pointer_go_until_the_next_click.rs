use super::*;

#[test]
fn ctrl_shift_escape_lets_the_pointer_go_until_the_next_click() {
    let (mut space, now) = space();
    assert!(space.grabbed(), "space holds the pointer from the start");
    send(&mut space, now, [Input::PointerMoved(centre())]);

    send(&mut space, now, steer(true));
    let escaped = send(
        &mut space,
        now,
        [key(Key::Escape, 1, true), key(Key::Escape, 1, false)],
    );
    send(&mut space, now, steer(false));
    assert_eq!(delivered(&escaped), [], "the app does not see the Escape");
    assert!(!space.grabbed(), "Ctrl+Shift+Escape lets the pointer go");

    let at = centre() + vec2(40.0, 0.0);
    let moved = delivered(&send(&mut space, now, [Input::PointerMoved(at)]));
    assert_eq!(moved.len(), 1, "a free pointer still hovers the app");
    let clicked = send(&mut space, now, [button(at, true), button(at, false)]);
    assert_eq!(delivered(&clicked), [], "the click that grabs it again is swallowed");
    assert!(space.grabbed());

    let got = delivered(&send(&mut space, now, [button(at, true), button(at, false)]));
    assert_eq!(got.len(), 2, "the next click reaches the app: {got:?}");

    let output = app_frame(CursorIcon::Default);
    assert!(
        space
            .frame(&output, &view(now))
            .window
            .window
            .requests
            .pointer_locked,
        "while grabbed it asks for pointer lock"
    );
}
