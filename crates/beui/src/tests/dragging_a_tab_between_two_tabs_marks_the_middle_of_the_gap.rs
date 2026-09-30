use super::*;
use crate::painter::Shape;

#[test]
fn dragging_a_tab_between_two_tabs_marks_the_middle_of_the_gap() {
    let (document, dock) = dock_of(3);
    let mut harness = Harness::sized(document, WIDE_VIEWPORT);
    harness.frame(Vec::new());
    let first_close = harness.rect(harness.find("dock.tab.1.close"));
    let second = harness.rect(dock_tab(harness.document(), dock, "Tab 2"));
    let accent = harness.document().theme().accent;

    let from = harness.center(dock_tab(harness.document(), dock, "Tab 3"));
    let over = pos2(second.left() - 2.0, second.center().y);
    harness.frame(vec![Event::PointerMoved(from)]);
    harness.frame(vec![Event::PointerButton {
        pos: from,
        button: PointerButton::Primary,
        pressed: true,
        modifiers: Modifiers::NONE,
    }]);
    harness.frame(vec![Event::PointerMoved(over)]);
    let output = harness.frame(Vec::new());

    let marker = output
        .shapes()
        .iter()
        .find_map(|shape| match shape {
            Shape::Rect { rect, color, .. }
                if rect.width() < 10.0
                    && rect.height() > 10.0
                    && color.to_array()[..3] == accent.to_array()[..3] =>
            {
                Some(*rect)
            }
            _ => None,
        })
        .expect("the tab bar marks where the dragged tab would land");
    let gap = (first_close.right() + second.left()) / 2.0;
    assert!(
        (marker.center().x - gap).abs() < 1.0,
        "the marker sits in the middle of the gap between the two tabs, at {gap}, not at {}",
        marker.center().x
    );
}
