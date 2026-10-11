use super::*;
use crate::input::CursorIcon;
use crate::unstyled::dock_state;

#[test]
fn a_press_just_outside_a_windows_border_resizes_it() {
    let (document, dock) = dock_of(2);
    let mut harness = Harness::sized(document, WIDE_VIEWPORT);
    let window = floated_window(&mut harness, dock);
    let rect = |harness: &Harness| {
        let state = dock_state(harness.document(), dock);
        let window = state.window_rect(window).expect("the window is open");
        window.translate(harness.rect(dock).min.to_vec2())
    };
    let before = rect(&harness);

    let beside = pos2(before.right() + 3.0, before.center().y);
    let output = harness.frame(vec![Event::PointerMoved(beside)]);
    assert_eq!(output.cursor_icon, CursorIcon::ResizeHorizontal);
    let corner = before.max + vec2(3.0, 3.0);
    let output = harness.frame(vec![Event::PointerMoved(corner)]);
    assert_eq!(output.cursor_icon, CursorIcon::ResizeNwSe);
    let above = pos2(before.right() + 3.0, before.top() - 3.0);
    let output = harness.frame(vec![Event::PointerMoved(above)]);
    assert_eq!(output.cursor_icon, CursorIcon::ResizeNeSw);

    harness.drag(beside, beside + vec2(40.0, 0.0));
    let widened = rect(&harness);
    assert_eq!(
        widened,
        Rect::from_min_max(before.min, before.max + vec2(40.0, 0.0)),
        "a press just outside the right edge drags the right edge"
    );

    let far = pos2(widened.right() + 12.0, widened.center().y);
    let output = harness.frame(vec![Event::PointerMoved(far)]);
    assert_ne!(output.cursor_icon, CursorIcon::ResizeHorizontal);
    harness.drag(far, far + vec2(40.0, 0.0));
    assert_eq!(
        rect(&harness),
        widened,
        "a press farther out leaves the window alone"
    );
}
