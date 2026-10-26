use super::*;

fn tilt(panel: &Panel) -> f32 {
    (panel.pose.rotation * Vec3::X).y
}

#[test]
fn dragging_the_roll_control_rolls_the_panel_and_the_rotate_control_never_does() {
    let (mut space, now) = space();
    let start = *main(&space);
    let grab = on_part(&space, WindowId::MAIN, Part::Roll);
    let mut path = vec![Input::PointerMoved(grab), button(grab, true)];
    for step in 0..=16 {
        let angle = step as f32 / 16.0 * std::f32::consts::TAU;
        path.push(Input::PointerMoved(
            grab + vec2(angle.sin() * 60.0, (1.0 - angle.cos()) * 60.0),
        ));
    }
    let turn = on_part(&space, WindowId::MAIN, Part::Turn);
    send(
        &mut space,
        now,
        [Input::PointerMoved(turn), button(turn, true)],
    );
    for input in &path[2..] {
        send(&mut space, now, [input.clone()]);
        assert!(
            tilt(main(&space)).abs() < 1e-4,
            "the rotate control only yaws and pitches, even dragged in a circle"
        );
    }
    send(&mut space, now, [button(turn, false)]);
    space.panels[0] = start;

    let to = grab + vec2(150.0, 0.0);
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
    assert_eq!(placed(&sent).len(), 1);
    let panel = *main(&space);
    assert!(
        close(panel.pose.position, start.pose.position),
        "it rolls about its centre"
    );
    assert!(
        close(panel.normal(), start.normal()),
        "rolling keeps it facing the same way"
    );
    assert!(
        tilt(&panel) > 0.1,
        "the control below the centre dragged right swings the bottom right, raising the right edge: {}",
        tilt(&panel)
    );

    let middle = centre();
    send(&mut space, now, [Input::PointerMoved(middle)]);
    send(&mut space, now, [button(middle, true)]);
    space.request(&WindowRequest::BeginMove(WindowId::MAIN));
    send(&mut space, later(now, 1.0), [button(middle, false)]);
    assert!(
        tilt(main(&space)).abs() < 1e-4,
        "a carry stands it upright again"
    );
}
