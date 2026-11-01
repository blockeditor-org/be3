use super::*;
use crate::dock::unstyled::DockApi;
use beui::CursorIcon;

#[test]
fn a_press_just_outside_a_windows_border_resizes_it() {
    let _root = crate::reactive::RootScope::new();
    let (document, dock) = dock_of(2);
    let mut harness = Harness::sized(document, WIDE_VIEWPORT);
    let window = floated_window(&mut harness, dock);
    let rect = |harness: &Harness| {
        let state = harness.document().api::<DockApi>(dock).state();
        let window = state.window_rect(window).expect("the window is open");
        window.translate(harness.rect(dock).min.to_vec2())
    };
    let before = rect(&harness);

    let beside = pos2(before.right() + 3.0, before.center().y);
    let output = harness.frame(vec![Input::PointerMoved(beside)]);
    assert_eq!(
        output.main().window.requests.cursor,
        CursorIcon::ResizeHorizontal
    );
    let corner = before.max + vec2(3.0, 3.0);
    let output = harness.frame(vec![Input::PointerMoved(corner)]);
    assert_eq!(output.main().window.requests.cursor, CursorIcon::ResizeNwSe);
    let above = pos2(before.right() + 3.0, before.top() - 3.0);
    let output = harness.frame(vec![Input::PointerMoved(above)]);
    assert_eq!(output.main().window.requests.cursor, CursorIcon::ResizeNeSw);

    harness.drag(beside, beside + vec2(40.0, 0.0));
    let widened = rect(&harness);
    assert_eq!(
        widened,
        Rect::from_min_max(before.min, before.max + vec2(40.0, 0.0)),
        "a press just outside the right edge drags the right edge"
    );

    let far = pos2(widened.right() + 12.0, widened.center().y);
    let output = harness.frame(vec![Input::PointerMoved(far)]);
    assert_ne!(
        output.main().window.requests.cursor,
        CursorIcon::ResizeHorizontal
    );
    harness.drag(far, far + vec2(40.0, 0.0));
    assert_eq!(
        rect(&harness),
        widened,
        "a press farther out leaves the window alone"
    );
}
