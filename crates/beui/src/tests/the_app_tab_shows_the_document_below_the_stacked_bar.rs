use super::*;

#[test]
fn the_app_tab_shows_the_document_below_the_stacked_bar() {
    let _root = crate::reactive::RootScope::new();
    let HelloColumn {
        document, padding, ..
    } = hello_column();
    let mut harness = Harness::new(document);

    harness.toggle_inspector();
    harness.show_inspector_tab(crate::inspector::InspectorTab::App);

    let app = harness.app_rect();
    let content = harness.rect(padding);
    assert_eq!(content.width(), VIEWPORT.x);
    assert!(app.top() > 0.0, "the stacked bar stays above the app");
    assert!(content.top() >= app.top());
    assert!(
        harness
            .inspector()
            .document
            .find_test_id("dock.pin.inspector.close")
            .is_none(),
        "the app's bar offers no close button"
    );

    harness.show_inspector_tab(crate::inspector::InspectorTab::Beui);
    harness.click(harness.close_button_center());
    harness.frame(Vec::new());
    assert!(
        !harness.inspector_open(),
        "a tool's bar closes the inspector"
    );
}
