use super::*;

#[test]
fn a_repaint_that_starts_while_another_scans_in_does_not_restart_it() {
    let panels = stacked_panels();
    let (upper, lower) = (panels.upper, panels.lower);
    let mut harness = Harness::sized(panels.document, WIDE_VIEWPORT);
    harness.toggle_inspector();
    harness.click(harness.performance_tab_center());
    harness.frame(Vec::new());
    harness.click(harness.slow_repaint_toggle_center());
    harness.settle();

    let (above, below) = (harness.rect(upper), harness.rect(lower));
    harness
        .document_mut()
        .set_frame_color(lower, Color32::from_gray(90));
    for _ in 0..5 {
        harness.frame(Vec::new());
    }
    harness
        .document_mut()
        .set_frame_color(upper, Color32::from_gray(200));
    harness
        .document_mut()
        .set_frame_color(lower, Color32::from_gray(120));
    let output = harness.frame(Vec::new());
    let damaged = output.damaged().unwrap_or_default();
    let band = |within: Rect| {
        damaged
            .rects()
            .iter()
            .map(|rect| rect.intersect(within))
            .find(|rect| rect.is_positive())
    };

    let started = band(above).expect("the new repaint starts scanning at once");
    assert_eq!(started.top(), above.top());
    let continued = band(below).expect("the earlier repaint keeps scanning");
    assert!(continued.top() > below.top());
}
