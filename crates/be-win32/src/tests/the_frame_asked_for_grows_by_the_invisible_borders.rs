use super::*;

#[test]
fn the_frame_asked_for_grows_by_the_invisible_borders() {
    let window = Rect::new(93, 100, 907, 707);
    let frame = Rect::new(100, 100, 900, 700);
    assert_eq!(window_size_for_frame(window, frame, 400, 300), (414, 307));
    assert_eq!(window_size_for_frame(frame, frame, 400, 300), (400, 300));
    assert_eq!(window_size_for_frame(window, frame, -50, 0), (1, 7));
}
