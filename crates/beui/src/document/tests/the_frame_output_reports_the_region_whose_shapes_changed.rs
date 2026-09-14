use super::*;

#[test]
fn the_frame_output_reports_the_region_whose_shapes_changed() {
    let panels = stacked_panels();
    let lower = panels.lower;
    let mut harness = Harness::new(panels.document);
    harness.frame(Vec::new());
    assert_eq!(harness.frame(Vec::new()).damage(), None);

    let repainted = harness.rect(lower);
    harness
        .document_mut()
        .set_frame_color(lower, Color32::from_gray(90));

    assert_eq!(harness.frame(Vec::new()).damage(), Some(repainted));
}
