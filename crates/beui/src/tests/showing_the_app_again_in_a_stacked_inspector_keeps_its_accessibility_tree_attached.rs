use super::*;

#[test]
fn showing_the_app_again_in_a_stacked_inspector_keeps_its_accessibility_tree_attached() {
    let _root = crate::reactive::RootScope::new();
    let HelloColumn { document, .. } = hello_column();
    let mut harness = Harness::sized(document, WIDE_VIEWPORT);

    harness.toggle_inspector();
    harness.settle();
    harness.resize(VIEWPORT);
    harness.settle();
    assert!(harness.inspector().tools_stacked());
    harness.show_inspector_tab(crate::inspector::InspectorTab::App);
    harness.show_inspector_tab(crate::inspector::InspectorTab::Beui);
    harness.show_inspector_tab(crate::inspector::InspectorTab::App);

    assert!(harness.inspector().app_visible());
    let lines = harness
        .context()
        .kept_accessibility()
        .expect("the harness keeps the accessibility tree")
        .rendered()
        .lines()
        .len();
    assert!(lines > 1, "the accessibility tree was published");
}
