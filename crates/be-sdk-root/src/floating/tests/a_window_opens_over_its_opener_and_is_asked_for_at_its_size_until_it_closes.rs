use super::*;

#[test]
fn a_window_opens_over_its_opener_and_is_asked_for_at_its_size_until_it_closes() {
    let mut windows = Vec::new();
    assert!(hear(
        &mut windows,
        &[main_screen(true)],
        vec![open(7, 1, pos2(40.0, 30.0))],
        None,
    )
    .0);
    let [window] = windows.as_slice() else {
        panic!("one window floats: {windows:?}");
    };
    // Where its placement says, over its opener, at the size it opened at.
    assert_eq!(
        window.rect,
        Rect::from_min_size(pos2(40.0, 30.0), vec2(200.0, 100.0))
    );
    // Asked for as a window of its own, with the screen it opened with, at its opener's pixels
    // per point, anchoring its views from where it shows, and opening windows of its own.
    let request = window.request();
    assert_eq!(request.screen, ScreenId::opened(1));
    assert_eq!(
        (request.instance, request.view),
        (EDITOR, ViewId(7))
    );
    assert_eq!(request.placement, ViewPlacement::Host(HostView::Window));
    assert_eq!(
        (request.metrics.pixel_width, request.metrics.pixel_height),
        (400, 200)
    );
    assert!(request.windows);
    assert_eq!(
        request.anchor_space.map(|space| space.to_host.origin),
        Some(pos2(40.0, 30.0))
    );
    assert!(hear(
        &mut windows,
        &[main_screen(true)],
        vec![WindowRequest::Close(ViewId(7))],
        None,
    )
    .0);
    assert!(windows.is_empty());
}
