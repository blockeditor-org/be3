use super::the_chrome_around_a_curved_panel::window;
use super::*;

#[test]
fn the_cursor_is_drawn_only_while_the_pointer_is_grabbed() {
    let (mut space, now, context) = painted_space();
    let output = app_frame(CursorIcon::Text);
    let windows = [window(&context, WindowId::MAIN, PLATFORM, "Main")];
    send(&mut space, now, [Input::PointerMoved(centre())]);
    assert!(
        space.scene(&windows, &view(now)).cursor.is_some(),
        "while it holds the pointer, space draws its own cursor"
    );
    let hosted = space.frame(&output, &view(now)).window.window.requests;
    assert_eq!(
        (hosted.cursor, hosted.pointer_locked),
        (CursorIcon::None, true)
    );

    send(&mut space, now, [Input::PointerUnlocked]);
    assert!(!space.grabbed());
    let hosted = space.frame(&output, &view(now)).window.window.requests;
    assert_eq!(
        (hosted.cursor, hosted.pointer_locked),
        (CursorIcon::Text, false),
        "let go, the platform's cursor shows the icon the window under it asks for"
    );
    assert!(
        space.scene(&windows, &view(now)).cursor.is_none(),
        "and the scene draws none"
    );
    assert_scene(
        &mut space,
        now,
        &windows,
        "the_cursor_is_drawn_only_while_the_pointer_is_grabbed",
    );

    send(
        &mut space,
        now,
        [button(centre(), true), button(centre(), false)],
    );
    assert!(space.grabbed());
    assert!(
        space.scene(&windows, &view(now)).cursor.is_some(),
        "the click that grabs it again draws it again"
    );
}
