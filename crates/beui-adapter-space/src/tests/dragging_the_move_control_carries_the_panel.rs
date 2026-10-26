use super::*;

#[test]
fn dragging_the_move_control_carries_the_panel() {
    let (mut space, now) = space();
    let window = open_ahead(&mut space, now, 0.4);
    let grab = on_part(&space, window, Part::Move);
    assert_eq!(
        space.cursor_icon(&app_frame(CursorIcon::Default)),
        CursorIcon::Default
    );
    send(&mut space, now, [Input::PointerMoved(grab)]);
    assert_eq!(
        space.cursor_icon(&app_frame(CursorIcon::Default)),
        CursorIcon::Grab
    );
    let pressed = send(&mut space, now, [button(grab, true)]);
    assert!(
        delivered(&pressed)
            .iter()
            .all(|(_, input)| *input == Input::PointerLeft),
        "the app sees no press"
    );
    assert_eq!(space.carry.as_ref().map(|carry| carry.window), Some(window));
    assert_eq!(
        space.cursor_icon(&app_frame(CursorIcon::Default)),
        CursorIcon::Grabbing
    );

    let before = space.panel(window).unwrap().pose.position;
    let moved = grab + vec2(-150.0, -40.0);
    assert_eq!(
        delivered(&send(&mut space, now, [Input::PointerMoved(moved)])),
        []
    );
    send(&mut space, now, [scroll(-2.0)]);
    let after = space.panel(window).unwrap().pose.position;
    assert!(
        after.x < before.x - 0.05,
        "it follows the cursor: {after:?}"
    );

    let released = send(&mut space, later(now, 1.0), [button(moved, false)]);
    assert_eq!(space.carry, None);
    assert_eq!(delivered(&released), [], "nor the release");
    assert_eq!(placed(&released).len(), 1);
    assert_eq!(space.chrome.gesture, None);
}
