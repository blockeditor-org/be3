use super::*;

#[test]
fn slowing_repaints_scans_a_change_in_from_its_top_edge() {
    let panels = stacked_panels();
    let lower = panels.lower;
    let mut harness = Harness::sized(panels.document, WIDE_VIEWPORT);
    harness.toggle_inspector();
    harness.click(harness.performance_tab_center());
    harness.frame(Vec::new());
    harness.click(harness.slow_repaint_toggle_center());
    harness.settle();

    let changed = harness.rect(lower);
    harness
        .document_mut()
        .set_frame_color(lower, Color32::from_gray(90));
    let mut scanned: Vec<Rect> = Vec::new();
    loop {
        let output = harness.frame(Vec::new());
        let damaged = output.damaged().unwrap_or_default();
        scanned.extend(
            damaged
                .rects()
                .iter()
                .filter(|band| band.intersects(changed)),
        );
        if !output.repaint {
            break;
        }
        assert!(
            scanned.len() < 600,
            "the repaint never finished scanning in"
        );
    }

    assert!(scanned.len() > 10);
    assert!(
        scanned
            .iter()
            .all(|band| band.height() < changed.height() / 4.0)
    );
    assert!(
        scanned
            .windows(2)
            .all(|pair| pair[0].top() <= pair[1].top())
    );
    let covered = scanned
        .iter()
        .fold(Rect::NOTHING, |covered, band| covered.union(*band));
    assert_eq!(covered, changed);
}
