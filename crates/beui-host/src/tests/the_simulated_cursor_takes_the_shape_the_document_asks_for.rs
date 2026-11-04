use super::*;
use crate::painter::Shape;
use crate::styled::*;
use beui::CursorIcon;

#[test]
fn the_simulated_cursor_takes_the_shape_the_document_asks_for() {
    let (document, [input]) = toolbar_of(|| {
        [view! {
            <TextInput value=String::new() />
        }]
    });
    let mut harness = Harness::sized(document, WIDE_VIEWPORT);
    harness.frame(Vec::new());
    harness.enable_mouse_simulation();
    let target = harness.center(input);
    harness.point_at(target);

    let output = harness.frame(Vec::new());
    let cursor = harness.simulated_cursor();

    assert_eq!(output.main().window.requests.cursor, CursorIcon::Text);
    let beam = output.main().shapes().iter().any(|shape| {
        matches!(shape, Shape::Line { from, to, .. }
            if from.x == cursor.x && to.x == cursor.x && to.y - from.y > 10.0)
    });
    assert!(beam, "the simulated cursor is not a text beam");
}
