use super::*;

#[test]
fn the_platform_dropping_the_lock_lets_the_pointer_go() {
    let (mut space, now) = space();
    send(&mut space, now, [Input::PointerMoved(centre())]);
    let dropped = delivered(&send(&mut space, now, [Input::PointerUnlocked]));
    assert_eq!(
        dropped,
        [(WindowId::MAIN, Input::PointerUnlocked)],
        "the window under the cursor hears the lock is gone, in case it held it"
    );
    assert!(
        !space.grabbed(),
        "a browser that let go of the lock on Escape lets the host's grab go too"
    );
    let output = app_frame(CursorIcon::Default);
    assert!(
        !space
            .frame(&output, &view(now))
            .window
            .window
            .requests
            .pointer_locked,
        "so it stops asking for a lock it does not hold"
    );

    let clicked = send(
        &mut space,
        now,
        [button(centre(), true), button(centre(), false)],
    );
    assert_eq!(
        delivered(&clicked),
        [],
        "the click that takes it back is swallowed"
    );
    assert!(space.grabbed());
    assert!(
        space
            .frame(&output, &view(now))
            .window
            .window
            .requests
            .pointer_locked
    );
}
