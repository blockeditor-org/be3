use super::*;

#[test]
fn clicking_the_inspector_close_button_closes_the_panel() {
    let HelloColumn { document, .. } = hello_column();
    let mut harness = Harness::new(document);

    harness.toggle_inspector();
    assert!(harness.document.inspector.is_some());

    harness.click(harness.close_button_center());
    harness.frame(Vec::new());

    assert!(harness.document.inspector.is_none());
}
