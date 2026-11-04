use super::*;

#[test]
fn the_inspector_shows_the_accesskit_tree() {
    let _root = crate::reactive::RootScope::new();
    let HelloColumn { document, .. } = hello_column();
    let mut harness = Harness::sized(document, WIDE_VIEWPORT);
    harness.toggle_inspector();

    harness.show_inspector_tab(crate::inspector::InspectorTab::AccessKit);
    harness.frame(Vec::new());

    assert_eq!(harness.tree(), ["GenericContainer", "  Label"]);
    assert_eq!(harness.inspector().entries[1].detail, "Hello");
}
