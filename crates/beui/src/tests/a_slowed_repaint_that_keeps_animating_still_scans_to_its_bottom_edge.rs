use super::*;

#[test]
fn a_slowed_repaint_that_keeps_animating_still_scans_to_its_bottom_edge() {
    let panels = stacked_panels();
    let lower = panels.lower;
    let mut harness = Harness::sized(panels.document, WIDE_VIEWPORT);
    harness.toggle_inspector();
    harness.click(harness.performance_tab_center());
    harness.frame(Vec::new());
    harness.click(harness.slow_repaint_toggle_center());
    harness.settle();

    let changed = harness.rect(lower);
    let mut scanned: Vec<Rect> = Vec::new();
    for frame in 0..90u8 {
        harness
            .document_mut()
            .set_frame_color(lower, Color32::from_gray(60 + frame));
        let output = harness.frame(Vec::new());
        scanned.extend(
            output
                .damaged()
                .unwrap_or_default()
                .rects()
                .iter()
                .filter(|band| band.intersects(changed)),
        );
    }

    assert!(
        scanned
            .iter()
            .all(|band| band.height() < changed.height() / 4.0)
    );
    let finished = scanned
        .iter()
        .position(|band| band.bottom() == changed.bottom())
        .expect("the scan reaches the bottom of a panel that keeps changing");
    assert_eq!(scanned[finished + 1].top(), changed.top());
}
