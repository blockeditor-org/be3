use super::*;

#[test]
fn a_panel_between_two_damaged_ones_is_left_alone() {
    let panels = three_panels();
    let (top, middle, bottom) = (panels.top, panels.middle, panels.bottom);
    let mut document = panels.document;
    let (_, paints) = counted(&mut document, middle);
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    let settled = paints.get();

    harness
        .document_mut()
        .set_frame_color(top, Color32::from_gray(10));
    harness
        .document_mut()
        .set_frame_color(bottom, Color32::from_gray(200));
    let output = harness.frame(Vec::new());

    assert_eq!(
        paints.get(),
        settled,
        "damage above and below an element must not be joined into one region across it"
    );
    let damage = output.damage().expect("both panels were repainted");
    assert!(damage.intersects(harness.rect(top)));
    assert!(damage.intersects(harness.rect(bottom)));
}
