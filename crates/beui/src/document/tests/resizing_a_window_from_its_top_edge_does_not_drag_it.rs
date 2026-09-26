use super::*;
use crate::unstyled::dock_state;

#[test]
fn resizing_a_window_from_its_top_edge_does_not_drag_it() {
    let (document, dock) = dock_of(2);
    let mut harness = Harness::sized(document, WIDE_VIEWPORT);
    harness.document_mut().set_rubber_banding(false);
    let window = floated_window(&mut harness, dock);
    let floated = dock_state(harness.document(), dock)
        .window_rect(window)
        .expect("the window has a rect");
    let bounds = harness.rect(dock);
    let bar = bounds.min + floated.min.to_vec2() + vec2(floated.width() - 80.0, 10.0);
    harness.drag(bar, bar - vec2(floated.min.x, 0.0));
    harness.document_mut().set_rubber_banding(true);
    let before = dock_state(harness.document(), dock)
        .window_rect(window)
        .expect("the window is still open");
    assert_eq!(before.min.x, 0.0);
    let tab = harness.rect(dock_tab(harness.document(), dock, "Tab 2"));

    let edge = bounds.min + before.min.to_vec2() + vec2(before.width() / 2.0, 2.0);
    harness.press_at(edge);
    harness.frame(vec![Event::PointerMoved(edge + vec2(-150.0, -30.0))]);

    let after = dock_state(harness.document(), dock)
        .window_rect(window)
        .expect("the window is still open");
    assert_eq!(
        after,
        Rect::from_min_max(before.min - vec2(0.0, 30.0), before.max),
        "dragging the top edge only moves the top edge"
    );
    assert_eq!(
        harness
            .rect(dock_tab(harness.document(), dock, "Tab 2"))
            .min
            .x,
        tab.min.x,
        "dragging the top edge sideways against the dock's edge does not stretch the window"
    );
}
