use super::*;

#[test]
fn the_inspector_shows_document_performance() {
    let _root = crate::reactive::RootScope::new();
    let HelloColumn { document, .. } = hello_column();
    let mut harness = Harness::sized(document, WIDE_VIEWPORT);
    harness.toggle_inspector();

    harness.show_inspector_tab(crate::inspector::InspectorTab::Performance);
    harness.frame(Vec::new());

    assert!(harness.performance_panel_visible());
    assert!(harness.inspector().entries.is_empty());
    assert!(harness.document().performance().samples > 0);
}
