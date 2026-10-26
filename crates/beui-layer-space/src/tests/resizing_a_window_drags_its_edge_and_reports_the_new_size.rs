use super::*;

#[test]
fn resizing_a_window_drags_its_edge_and_reports_the_new_size() {
    let (mut space, now) = space();
    let window = open_ahead(&mut space, now, 0.5);
    let start = space.panel(window).unwrap().clone();
    let corner = start.world(Pos2::ZERO);
    let grip = centre() + vec2(100.0, 75.0);
    let gripped = on(&space, window, grip);
    send(
        &mut space,
        now,
        [Input::PointerMoved(grip), button(grip, true)],
    );
    space.request(&WindowRequest::BeginResize(window, WindowEdge::BottomRight));

    let dragged = grip + vec2(60.0, 30.0);
    let sent = send(&mut space, now, [Input::PointerMoved(dragged)]);
    assert_eq!(
        delivered(&sent),
        [],
        "the app sees no moves while it resizes"
    );
    let panel = space.panel(window).unwrap().clone();
    let along = start
        .intersect(space.head.ray(PLATFORM, dragged.to_vec2()))
        .unwrap()
        .pos
        - gripped;
    assert!(along.x > 50.0 && along.y > 20.0, "{along:?}");
    let size = vec2((SMALL.x + along.x).round(), (SMALL.y + along.y).round());
    assert_eq!(panel.size, size, "the corner follows the cursor");
    assert_eq!(
        reports(&sent),
        [WindowReport::Resized { window, size }],
        "the app hears the new size"
    );
    assert!(
        close(panel.world(Pos2::ZERO), corner),
        "the opposite corner stays where it was"
    );
    assert!(panel.pose.rotation.angle_between(start.pose.rotation) < 1e-6);

    let released = send(&mut space, now, [button(dragged, false)]);
    assert_eq!(space.resize, None, "letting go ends the resize");
    let got = delivered(&released);
    let [(to, Input::PointerButton { pressed: false, .. })] = got.as_slice() else {
        panic!("the app gets the release, not {got:?}");
    };
    assert_eq!(*to, window);
    assert_eq!(placed(&released).len(), 1, "and the panel's new place");

    space.request(&WindowRequest::BeginResize(window, WindowEdge::Left));
    let shrunk = send(
        &mut space,
        now,
        [Input::PointerMoved(centre() + vec2(500.0, 0.0))],
    );
    assert_eq!(
        reports(&shrunk),
        [WindowReport::Resized {
            window,
            size: vec2(crate::resize::SMALLEST.x, size.y)
        }],
        "a window never shrinks below the smallest size"
    );
}
