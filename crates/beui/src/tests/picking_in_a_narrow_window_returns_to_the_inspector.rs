use super::*;

#[test]
fn picking_in_a_narrow_window_returns_to_the_inspector() {
    let HelloColumn { document, text, .. } = hello_column();
    let mut harness = Harness::new(document);

    harness.toggle_inspector();
    harness.toggle_picking();
    assert!(harness.inspector().state.app_visible());

    harness.click(harness.center(text));
    harness.frame(Vec::new());

    assert_eq!(harness.inspector().state.selected.get(), Some(text));
    assert!(!harness.inspector().state.app_visible());
}
