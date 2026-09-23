use super::*;

#[test]
fn the_inspector_shows_the_accesskit_tree() {
    let HelloColumn { document, .. } = hello_column();
    let mut harness = Harness::sized(document, WIDE_VIEWPORT);
    harness.toggle_inspector();

    harness.click(harness.accesskit_tab_center());
    harness.frame(Vec::new());

    assert_eq!(harness.tree(), ["GenericContainer", "  Label"]);
    assert_eq!(harness.inspector().entries[1].detail, "Hello");
}
