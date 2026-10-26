use super::*;

#[test]
fn the_cursor_shows_the_icon_the_window_under_it_asks_for_and_hides_the_platform_one() {
    let (mut space, now) = space();
    let output = app_frame(CursorIcon::Text);
    send(&mut space, now, [Input::PointerMoved(centre())]);

    assert_eq!(
        space.cursor_icon(&output),
        CursorIcon::Text,
        "over the panel the cursor takes the window's icon"
    );
    let hosted = space.frame(&output, &view(now));
    assert_eq!(
        hosted.window.window.requests.cursor,
        CursorIcon::None,
        "the platform's cursor is hidden, the scene draws its own"
    );

    send(&mut space, now, [Input::PointerMoved(pos2(2.0, 2.0))]);
    assert_eq!(
        space.cursor_icon(&output),
        CursorIcon::Default,
        "off every panel it is an arrow"
    );
    assert_eq!(
        space
            .frame(&output, &view(now))
            .window
            .window
            .requests
            .cursor,
        CursorIcon::None
    );
}
