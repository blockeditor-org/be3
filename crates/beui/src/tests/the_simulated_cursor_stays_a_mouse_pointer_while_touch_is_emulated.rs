use super::*;
use crate::input::CursorIcon;
use crate::painter::Shape;

#[test]
fn the_simulated_cursor_stays_a_mouse_pointer_while_touch_is_emulated() {
    let _root = crate::reactive::RootScope::new();
    let mut harness = Harness::sized(hello_column().document, WIDE_VIEWPORT);
    harness.frame(Vec::new());
    harness.toggle_inspector();
    harness.show_inspector_tab(crate::inspector::InspectorTab::Simulation);
    harness.frame(Vec::new());
    harness.click(harness.mouse_toggle_center());
    harness.click(harness.touch_toggle_center());
    harness.key(Key::Escape, Modifiers::NONE);
    harness.frame(Vec::new());
    assert!(harness.touch_emulation());

    let output = harness.frame(Vec::new());
    let cursor = harness.simulated_cursor();

    assert_eq!(output.main().window.requests.cursor, CursorIcon::Touch);
    let ring = output.main().shapes().iter().any(|shape| {
        matches!(shape, Shape::Rect { rect, stroke_width, .. }
            if *stroke_width == crate::native::touch_cursor::STROKE
                && (rect.center() - cursor).length() < 0.01)
    });
    assert!(!ring, "the simulated cursor is the touch circle");
    let arrow = output.main().shapes().iter().any(|shape| {
        matches!(shape, Shape::Text { origin, .. }
            if origin.x < cursor.x
                && origin.y < cursor.y
                && (*origin - cursor).length() < 16.0)
    });
    assert!(arrow, "the simulated cursor is not a mouse pointer");
}
