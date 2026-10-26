use super::the_chrome_around_a_curved_panel::window;
use super::*;

#[test]
fn dragging_the_rotate_control_keeps_the_cursor_on_the_button() {
    let (mut space, now, context) = painted_space();
    let start = *main(&space);
    let grab = on_part(&space, WindowId::MAIN, Part::Turn);
    send(
        &mut space,
        now,
        [Input::PointerMoved(grab), button(grab, true)],
    );

    let sent = send(&mut space, now, [Input::PointerMotion(vec2(80.0, 40.0))]);
    assert_eq!(delivered(&sent), [], "the app sees nothing of the control");
    let normal = main(&space).normal();
    assert!(
        normal.x > 0.1 && normal.y < -0.05,
        "motion right and down turns it right and tips its top toward the viewer: {normal:?}"
    );
    let on_button = on_part(&space, WindowId::MAIN, Part::Turn);
    assert!(
        (space.cursor_screen().unwrap() - on_button).length() < 0.5,
        "the cursor stays on the button: {:?} and {on_button:?}",
        space.cursor_screen()
    );

    send(&mut space, now, [Input::PointerMotion(vec2(-80.0, -40.0))]);
    assert!(
        main(&space).normal().angle_between(start.normal()) < 1e-3,
        "moving back turns it back"
    );

    send(&mut space, now, [Input::PointerMotion(vec2(-60.0, 50.0))]);
    let windows = [window(&context, WindowId::MAIN, PLATFORM, "Main")];
    assert_scene(
        &mut space,
        now,
        &windows,
        "dragging_the_rotate_control_keeps_the_cursor_on_the_button",
    );
    let on_button = on_part(&space, WindowId::MAIN, Part::Turn);
    let released = send(&mut space, now, [button(on_button, false)]);
    assert_eq!(
        placed(&released).len(),
        1,
        "letting go reports the placement"
    );
    assert!(
        (space.cursor_screen().unwrap() - on_button).length() < 0.5,
        "and leaves the cursor on the button"
    );
}
