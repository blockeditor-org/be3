use super::*;

#[test]
fn dragging_the_rotate_control_turns_the_panel_until_it_is_carried() {
    let (mut space, now) = space();
    let start = *main(&space);
    let grab = on_part(&space, WindowId::MAIN, Part::Turn);
    let to = grab + vec2(80.0, 40.0);
    let sent = send(
        &mut space,
        now,
        [
            Input::PointerMoved(grab),
            button(grab, true),
            Input::PointerMoved(to),
            button(to, false),
        ],
    );
    assert_eq!(delivered(&sent), [], "the app sees nothing of the control");
    assert_eq!(placed(&sent).len(), 1);
    let panel = *main(&space);
    assert!(
        close(panel.pose.position, start.pose.position),
        "it turns about its centre"
    );
    let normal = panel.normal();
    assert!(
        normal.x > 0.1,
        "dragging right turns it to the right: {normal:?}"
    );
    assert!(
        normal.y < -0.05,
        "dragging down tips its top toward the viewer: {normal:?}"
    );

    let middle = centre();
    send(&mut space, now, [Input::PointerMoved(middle)]);
    space.request(&WindowRequest::BeginMove(WindowId::MAIN));
    send(&mut space, now, [button(middle, true)]);
    send(&mut space, later(now, 1.0), [button(middle, false)]);
    let normal = main(&space).normal();
    assert!(
        normal.y.abs() < 1e-4,
        "a carry turns it back to face the viewer: {normal:?}"
    );
}
