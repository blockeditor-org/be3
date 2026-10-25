use super::*;

#[test]
fn a_driver_pointing_at_a_window_shows_its_icon() {
    let (mut space, now) = space();
    let mut incoming =
        vec![Incoming::points(Input::PointerMoved(pos2(20.0, 20.0))).in_window(WindowId::MAIN)];
    space.input(&mut incoming, &view(now));
    assert_eq!(
        space.cursor_icon(&app_frame(CursorIcon::Text)),
        CursorIcon::Text
    );
}
