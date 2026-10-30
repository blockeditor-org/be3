use super::*;

#[test]
fn a_window_moved_off_the_bottom_of_the_app_keeps_its_size_and_its_top_in_view() {
    let mut window = Carrying::open(pos2(100.0, 100.0), vec2(300.0, 200.0));
    let held = pos2(100.0, 100.0) + HELD;
    window.press(held);
    window.drag_to(held + vec2(300.0, 200.0));
    window.drag_to(held + vec2(600.0, 450.0));
    window.release(held + vec2(600.0, 450.0));
    window.frames_to_settle();
    assert_eq!(
        window.shown(),
        Rect::from_min_size(pos2(700.0, 550.0), vec2(300.0, 200.0)),
        "a window goes off the right and bottom edges at its full size, as long as its corner \
         and the point it was held by stay in view"
    );

    let held = pos2(700.0, 550.0) + HELD;
    window.press(held);
    window.drag_to(held + vec2(0.0, 200.0));
    window.release(held + vec2(0.0, 200.0));
    window.frames_to_settle();
    assert_eq!(
        window.shown(),
        Rect::from_min_size(
            pos2(700.0, APP.y - HELD.y - GRAB_MARGIN),
            vec2(300.0, 200.0)
        ),
        "pulled further, it springs back to keep the point it was held by in view"
    );
}
