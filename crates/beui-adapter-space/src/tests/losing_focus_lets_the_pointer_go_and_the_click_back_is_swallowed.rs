use super::*;

#[test]
fn losing_focus_lets_the_pointer_go_and_the_click_back_is_swallowed() {
    let (mut space, now) = space();
    send(&mut space, now, [Input::PointerMoved(centre())]);
    let blurred = delivered(&send(&mut space, now, [Input::Focus(false)]));
    assert_eq!(
        blurred,
        [(WindowId::MAIN, Input::Focus(false))],
        "the app hears it lost focus"
    );
    assert!(!space.grabbed(), "an unfocused window lets the pointer go");
    let output = app_frame(CursorIcon::Default);
    assert!(
        !space
            .frame(&output, &view(now))
            .window
            .window
            .requests
            .pointer_locked,
        "and stops asking for pointer lock"
    );

    send(&mut space, now, [Input::Focus(true)]);
    assert!(!space.grabbed(), "focus alone does not grab it");
    let clicked = send(
        &mut space,
        now,
        [button(centre(), true), button(centre(), false)],
    );
    assert_eq!(delivered(&clicked), [], "the click back into the window is swallowed");
    assert!(space.grabbed(), "and grabs the pointer again");
    assert!(
        space
            .frame(&output, &view(now))
            .window
            .window
            .requests
            .pointer_locked
    );
}
