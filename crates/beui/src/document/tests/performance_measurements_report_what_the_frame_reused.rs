use super::*;

#[test]
fn performance_measurements_report_what_the_frame_reused() {
    let panels = three_panels();
    let middle = panels.middle;
    let mut harness = Harness::new(panels.document);
    harness.frame(Vec::new());

    let first = harness.document().performance().latest.work;
    assert!(first.measured > 0, "the first frame has to measure");
    assert_eq!(
        first.reused_measurements, 0,
        "the first frame has nothing to reuse"
    );
    assert!(first.painted_nodes > 0);
    assert_eq!(first.replayed_nodes, 0);

    harness
        .document_mut()
        .set_frame_color(middle, Color32::from_gray(120));
    harness.frame(Vec::new());

    let after = harness.document().performance().latest.work;
    assert!(
        after.reused_measurements > after.measured,
        "recolouring one panel must reuse more measurements than it takes, got {after:?}"
    );
    assert!(
        after.replayed_nodes > 0,
        "the panels the change does not reach must replay their shapes, got {after:?}"
    );
}
