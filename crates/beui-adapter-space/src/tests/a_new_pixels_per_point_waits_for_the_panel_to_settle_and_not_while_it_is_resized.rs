use super::*;

#[test]
fn a_new_pixels_per_point_waits_for_the_panel_to_settle_and_not_while_it_is_resized() {
    let (mut space, now) = space();
    let window = open_ahead(&mut space, now, 1.0);
    let held = space
        .scale(window)
        .expect("a new panel is drawn at once at its scale");
    let rescaled = |sent: &[Incoming]| {
        scaled(sent)
            .into_iter()
            .filter(|(scaled, _)| *scaled == window)
            .collect::<Vec<_>>()
    };

    space.head.position.z -= 0.6;
    assert!(
        rescaled(&send(&mut space, now, [])).is_empty(),
        "a panel that needs more pixels per point does not change at once"
    );
    let waiting = space.frame(&app_frame(CursorIcon::Default), &view(now));
    assert!(
        waiting.repaint_after > Duration::ZERO && waiting.repaint_after <= crate::scale::SETTLE,
        "the host asks for a frame when the wait is over, not every frame: {:?}",
        waiting.repaint_after
    );
    let soon = later(now, crate::scale::SETTLE.as_secs_f32() / 2.0);
    assert!(rescaled(&send(&mut space, soon, [])).is_empty());
    let settled = later(now, crate::scale::SETTLE.as_secs_f32() + 0.01);
    let sent = send(&mut space, settled, []);
    let nearer = space.scale(window).expect("the panel has a scale");
    assert!(nearer > held);
    assert_eq!(
        rescaled(&sent),
        [(window, nearer)],
        "once it has wanted the same scale for a while, the app hears it"
    );

    let mut now = settled;
    let grip = centre() + vec2(100.0, 75.0);
    send(
        &mut space,
        now,
        [Input::PointerMoved(grip), button(grip, true)],
    );
    space.request(&WindowRequest::BeginResize {
        window,
        edge: WindowEdge::BottomRight,
    });
    space.head.position.z += 3.0;
    for step in 1..=20 {
        now = later(now, 0.1);
        let sent = send(
            &mut space,
            now,
            [Input::PointerMoved(grip + vec2(step as f32 * 4.0, 0.0))],
        );
        assert!(
            rescaled(&sent).is_empty(),
            "a panel being resized keeps its pixels per point, however long it takes"
        );
    }
    send(&mut space, now, [button(grip, false)]);
    assert_eq!(space.mouse.resize, None);
    assert!(rescaled(&send(&mut space, now, [])).is_empty());
    let sent = send(
        &mut space,
        later(now, crate::scale::SETTLE.as_secs_f32() + 0.01),
        [],
    );
    let farther = space.scale(window).expect("the panel has a scale");
    assert!(farther < nearer);
    assert_eq!(
        rescaled(&sent),
        [(window, farther)],
        "and takes the scale it needs once the resize is over and it has settled"
    );
}
