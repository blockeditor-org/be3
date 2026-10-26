use super::*;
use crate::dock::unstyled::DockApi;
use crate::painter::Shape;

#[test]
fn dragging_a_tab_over_a_window_bar_marks_where_it_lands() {
    let _root = crate::reactive::RootScope::new();
    let (document, dock) = dock_of(2);
    let mut harness = Harness::sized(document, WIDE_VIEWPORT);
    harness.frame(Vec::new());
    let tab = harness.center(dock_tab(harness.document(), dock, "Tab 2"));
    drag_with(
        &mut harness,
        tab,
        pos2(WIDE_VIEWPORT.x / 2.0, WIDE_VIEWPORT.y / 2.0),
        Modifiers::ALT,
    );
    harness.frame(Vec::new());
    let window = harness.document().api::<DockApi>(dock).state().windows()[0];
    let floated = dock_tab_extent(&harness, dock, "Tab 2", 2);
    let accent = Theme::of(harness.document().preferences().theme).accent;

    let tab = harness.center(dock_tab(harness.document(), dock, "Tab 1"));
    let over = pos2(floated.right() + 20.0, floated.center().y);
    harness.frame(vec![Input::PointerMoved(tab)]);
    harness.frame(vec![Input::PointerButton {
        pos: tab,
        button: PointerButton::Primary,
        pressed: true,
    }]);
    harness.frame(vec![Input::PointerMoved(over)]);
    let output = harness.frame(Vec::new());

    let bar = Rect::from_min_max(
        pos2(floated.right(), floated.top() - 20.0),
        pos2(floated.right() + 40.0, floated.bottom() + 20.0),
    );
    assert!(
        output.main().shapes().iter().any(|shape| matches!(
            shape,
            Shape::Rect { rect, color, .. }
                if bar.contains_rect(*rect) && color.to_array()[..3] == accent.to_array()[..3]
        )),
        "the bar of a window marks where the tab being dragged over it would land"
    );
    assert_eq!(
        harness.document().api::<DockApi>(dock).state().windows(),
        vec![window],
        "hovering a window's bar with a tab does not open another window"
    );
}
