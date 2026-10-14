use super::*;
use crate::input::CursorIcon;
use crate::painter::Shape;

#[test]
fn the_simulated_cursor_is_the_touch_cursor_while_touch_is_emulated() {
    let _root = crate::reactive::RootScope::new();
    let mut harness = Harness::sized(hello_column().document, WIDE_VIEWPORT);
    harness.frame(Vec::new());
    harness.toggle_inspector();
    harness.click(harness.simulation_tab_center());
    harness.frame(Vec::new());
    harness.click(harness.mouse_toggle_center());
    harness.click(harness.touch_toggle_center());
    harness.key(Key::Escape, Modifiers::NONE);
    harness.frame(Vec::new());
    assert!(harness.touch_emulation());

    let output = harness.frame(Vec::new());
    let cursor = harness.simulated_cursor();

    assert_eq!(output.window.cursor, CursorIcon::Touch);
    let ring = output.shapes().iter().any(|shape| {
        matches!(shape, Shape::Rect { rect, stroke_width, .. }
            if *stroke_width == crate::touch_cursor::STROKE
                && (rect.center() - cursor).length() < 0.01
                && (rect.width() - (crate::touch_cursor::SIZE - crate::touch_cursor::STROKE)).abs() < 0.01)
    });
    assert!(
        ring,
        "the simulated cursor is not the circle the platforms show"
    );
}
