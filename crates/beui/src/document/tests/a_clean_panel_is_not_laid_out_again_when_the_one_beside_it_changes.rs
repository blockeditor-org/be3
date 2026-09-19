use super::*;

#[test]
fn a_clean_panel_is_not_laid_out_again_when_the_one_beside_it_changes() {
    let panels = three_panels();
    let (top, middle) = (panels.top, panels.middle);
    let mut document = panels.document;
    let (layouts, _) = counted(&mut document, middle);
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    let settled = layouts.get();
    assert!(settled > 0, "the first frame has to lay the panel out");

    harness
        .document_mut()
        .set_frame_color(top, Color32::from_gray(10));
    harness.frame(Vec::new());
    assert_eq!(
        layouts.get(),
        settled,
        "a panel whose subtree did not change must keep the placement it had"
    );
    let work = harness.document().performance().latest.work;
    assert!(work.reused_placements > 0);

    harness
        .document_mut()
        .set_frame_color(middle, Color32::from_gray(200));
    harness.frame(Vec::new());
    assert!(
        layouts.get() > settled,
        "a panel that changed has to be laid out again"
    );
}
