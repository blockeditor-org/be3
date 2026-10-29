use super::*;

#[test]
fn picking_in_a_narrow_window_returns_to_the_inspector() {
    let _root = crate::reactive::RootScope::new();
    let HelloColumn { document, text, .. } = hello_column();
    let mut harness = Harness::new(document);

    harness.toggle_inspector();
    harness.toggle_picking();
    harness.settle();
    assert!(harness.inspector().app_visible(), "picking shows the app");

    harness.click(harness.center(text));
    harness.settle();

    assert_eq!(harness.inspector().state().selected.get(), Some(text));
    assert!(
        !harness.inspector().app_visible(),
        "a pick returns to the tool it came from"
    );
}
