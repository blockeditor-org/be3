use super::*;
use crate::flash;

#[test]
fn flashing_repaints_paints_no_fill() {
    let panels = stacked_panels();
    let lower = panels.lower;
    let mut harness = Harness::sized(panels.document, WIDE_VIEWPORT);
    harness.toggle_inspector();
    harness.click(harness.performance_tab_center());
    harness.frame(Vec::new());
    harness.click(harness.damage_flash_toggle_center());
    harness.frame(Vec::new());

    let repainted = harness.rect(lower);
    harness
        .document_mut()
        .set_frame_color(lower, Color32::from_gray(90));
    let output = harness.frame(Vec::new());

    assert!(flashed(&output, repainted, flash::REPAINT));
    assert!(!output.shapes().iter().any(|shape| matches!(
        shape,
        crate::painter::Shape::Rect {
            rect,
            stroke_width,
            color,
            ..
        } if *rect == repainted
            && *stroke_width == 0.0
            && color.to_array()[..3] == flash::REPAINT.to_array()[..3]
    )));
}
