use super::*;

#[test]
fn a_panel_between_two_damaged_ones_is_left_alone() {
    let _root = crate::reactive::RootScope::new();
    let panels = three_panels();
    let (top, middle, bottom) = (panels.top, panels.middle, panels.bottom);
    let mut document = panels.document;
    document.set_frame_height(top, Extent::px(40.0));
    document.set_frame_height(bottom, Extent::px(40.0));
    let (_, paints) = counted(&mut document, middle);
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    let settled = paints.get_untracked();

    harness
        .document_mut()
        .set_frame_color(top, Color32::from_gray(10));
    harness
        .document_mut()
        .set_frame_color(bottom, Color32::from_gray(200));
    let output = harness.frame(Vec::new());

    assert_eq!(
        paints.get_untracked(),
        settled,
        "damage above and below an element taller than both must not be joined into one region across it"
    );
    assert_eq!(
        output.main().damage.rects(),
        [harness.rect(top), harness.rect(bottom)],
        "the frame reports the two panels as separate regions, leaving the one between them out"
    );
}
