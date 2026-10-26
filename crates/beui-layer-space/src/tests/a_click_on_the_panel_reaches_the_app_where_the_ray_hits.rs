use super::*;

#[test]
fn a_click_on_the_panel_reaches_the_app_where_the_ray_hits() {
    let now = Instant::now();
    let mut space = Space::new();
    let mut incoming = Vec::new();
    space.input(&mut incoming, &view(now));
    assert_eq!(
        reports(&incoming),
        [WindowReport::Resized {
            window: WindowId::MAIN,
            size: PLATFORM,
        }],
        "the main window is sized like the platform window"
    );
    let panel = main(&space);
    assert!(
        panel.pose.position.z < space.head.position.z,
        "it is in front of the viewer"
    );

    let got = delivered(&send(
        &mut space,
        now,
        [
            Input::PointerMoved(centre()),
            button(centre(), true),
            button(centre(), false),
        ],
    ));
    let at = centre();
    assert_eq!(got.len(), 3);
    for (window, input) in &got {
        assert_eq!(*window, WindowId::MAIN);
        let pos = match input {
            Input::PointerMoved(pos) | Input::PointerButton { pos, .. } => *pos,
            other => panic!("unexpected {other:?}"),
        };
        assert!(
            (pos - at).length() < 0.01,
            "the middle of the view is the middle of the window, not {pos:?}"
        );
    }

    let typed = delivered(&send(&mut space, now, [key(Key::A, 30, true)]));
    assert_eq!(
        typed,
        [(WindowId::MAIN, key(Key::A, 30, true))],
        "keys go to the app"
    );
}
