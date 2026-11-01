use super::*;
use crate::dock::unstyled::DockApi;
use beui::CursorIcon;

#[test]
fn a_press_on_a_windows_border_resizes_it() {
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

    let border = pos2(before.right() - 1.0, before.center().y);
    let output = harness.frame(vec![Input::PointerMoved(border)]);
    assert_eq!(
        output.main().window.requests.cursor,
        CursorIcon::ResizeHorizontal,
        "the border shows the resize cursor"
    );
    harness.drag(border, border + vec2(40.0, 0.0));
    let widened = rect(&harness);
    assert_eq!(
        widened,
        Rect::from_min_max(before.min, before.max + vec2(40.0, 0.0)),
        "a press on the right border drags the right edge rather than moving the window"
    );

    let top = pos2(widened.right() - 80.0, widened.top() + 1.0);
    harness.drag(top, top + vec2(-30.0, -20.0));
    assert_eq!(
        rect(&harness),
        Rect::from_min_max(widened.min - vec2(0.0, 20.0), widened.max),
        "a press on the top border, above the bar, drags the top edge"
    );
}
