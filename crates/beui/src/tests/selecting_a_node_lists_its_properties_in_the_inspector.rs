use super::*;

#[test]
fn selecting_a_node_lists_its_properties_in_the_inspector() {
    let HelloColumn { document, .. } = hello_column();
    let mut harness = Harness::new(document);

    harness.toggle_inspector();
    let shows = |harness: &Harness, text: &str| {
        let inspector = harness.inspector();
        text_within(
            &inspector.document,
            inspector.find("inspector.properties"),
            text,
        )
        .is_some()
    };
    assert!(!shows(&harness, "Properties"));

    harness.click(harness.row_center(1));
    harness.frame(Vec::new());

    assert!(shows(&harness, "Properties"));
    assert!(shows(&harness, "padding"));
    assert!(shows(&harness, "4 4 4 4"));

    harness.click(harness.row_center(0));
    harness.frame(Vec::new());

    assert!(shows(&harness, "direction"));
    assert!(shows(&harness, "Vertical"));
    assert!(!shows(&harness, "4 4 4 4"));
}
