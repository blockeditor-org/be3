use super::*;
use crate::painter::Shape;

#[test]
fn hiding_the_text_behind_the_screen_reader_curtain_paints_none_of_it() {
    let mut harness = Harness::sized(hello_column().document, TALL_VIEWPORT);
    harness.frame(Vec::new());
    assert!(paints_text(&harness));

    harness.enable_screen_reader();
    harness.frame(Vec::new());
    assert!(!paints_text(&harness));

    harness.click(harness.screen_reader_control_center("hide_text"));
    harness.frame(Vec::new());
    assert!(paints_text(&harness));
}

fn paints_text(harness: &Harness) -> bool {
    harness
        .document()
        .shapes()
        .iter()
        .any(|shape| matches!(shape, Shape::Text { .. }))
}
