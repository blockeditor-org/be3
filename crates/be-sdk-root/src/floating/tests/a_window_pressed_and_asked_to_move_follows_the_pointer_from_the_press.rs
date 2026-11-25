use super::*;

#[test]
fn a_window_pressed_and_asked_to_move_follows_the_pointer_from_the_press() {
    // Two windows, the second on top. A press on the first raises it; the move its app asks for
    // then follows the pointer from where it pressed, as does a resize from its corner.
    let mut windows = Vec::new();
    let _ = hear(
        &mut windows,
        &[main_screen(true)],
        vec![open(7, 1, pos2(40.0, 30.0)), open(8, 2, pos2(60.0, 50.0))],
        None,
        Pos2::ZERO,
    );
    let pressed = press(&mut windows, ScreenId::opened(1), pos2(50.0, 35.0));
    assert!(pressed.is_some());
    let views = |windows: &[Floating]| windows.iter().map(|window| window.view).collect::<Vec<_>>();
    assert_eq!(views(&windows), [ViewId(8), ViewId(7)], "the pressed window comes in front");
    let (_, held) = hear(
        &mut windows,
        &[main_screen(true)],
        vec![WindowRequest::BeginMove(ViewId(7))],
        pressed,
        pos2(70.0, 45.0),
    );
    let held = held.expect("the move begins");
    assert!(drag(&mut windows, held, pos2(150.0, 135.0)));
    let rect = |windows: &[Floating], view| {
        windows
            .iter()
            .find(|window| window.view == view)
            .map(|window| window.rect)
    };
    assert_eq!(
        rect(&windows, ViewId(7)),
        Some(Rect::from_min_size(pos2(140.0, 130.0), vec2(200.0, 100.0)))
    );
    // Asked for where it moved to: its views anchor from there.
    let moved = windows
        .iter()
        .find(|window| window.view == ViewId(7))
        .map(Floating::request);
    assert_eq!(
        moved.and_then(|request| request.anchor_space).map(|space| space.to_host.origin),
        Some(pos2(140.0, 130.0))
    );
    let pressed = press(&mut windows, ScreenId::opened(1), pos2(70.0, 45.0));
    let (_, held) = hear(
        &mut windows,
        &[main_screen(true)],
        vec![WindowRequest::BeginResize(WindowResizeBegin {
            view: ViewId(7),
            edge: WindowEdge::BottomRight,
        })],
        pressed,
        pos2(70.0, 45.0),
    );
    let held = held.expect("the resize begins");
    assert!(drag(&mut windows, held, pos2(80.0, 65.0)));
    assert_eq!(
        rect(&windows, ViewId(7)),
        Some(Rect::from_min_size(pos2(140.0, 130.0), vec2(210.0, 120.0)))
    );
}
