use super::*;
use crate::flash;

#[test]
fn flashing_repaints_outlines_only_the_region_whose_shapes_changed() {
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
    assert!(!flashed(
        &output,
        harness.rect(panels.upper),
        flash::REPAINT
    ));
}
