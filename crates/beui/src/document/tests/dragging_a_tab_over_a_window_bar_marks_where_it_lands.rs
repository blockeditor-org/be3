use super::*;
use crate::painter::Shape;
use crate::unstyled::dock_state;

#[test]
fn dragging_a_tab_over_a_window_bar_marks_where_it_lands() {
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
    let window = dock_state(harness.document(), dock).windows()[0];
    let floated = harness.rect(dock_tab(harness.document(), dock, "Tab 2"));
    let accent = harness.document().theme().accent;

    let tab = harness.center(dock_tab(harness.document(), dock, "Tab 1"));
    let over = pos2(floated.right() + 20.0, floated.center().y);
    harness.frame(vec![Event::PointerMoved(tab)]);
    harness.frame(vec![Event::PointerButton {
        pos: tab,
        button: PointerButton::Primary,
        pressed: true,
        modifiers: Modifiers::NONE,
    }]);
    harness.frame(vec![Event::PointerMoved(over)]);
    let output = harness.frame(Vec::new());

    let bar = Rect::from_min_max(
        pos2(floated.right(), floated.top() - 20.0),
        pos2(floated.right() + 40.0, floated.bottom() + 20.0),
    );
    assert!(
        output.shapes().iter().any(|shape| matches!(
            shape,
            Shape::Rect { rect, color, .. }
                if bar.contains_rect(*rect) && color.to_array()[..3] == accent.to_array()[..3]
        )),
        "the bar of a window marks where the tab being dragged over it would land"
    );
    assert_eq!(
        dock_state(harness.document(), dock).windows(),
        vec![window],
        "hovering a window's bar with a tab does not open another window"
    );
}
