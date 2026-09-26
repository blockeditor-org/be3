use super::*;

#[test]
fn clicking_a_row_leaves_its_children_expanded() {
    let HelloColumn { document, .. } = hello_column();
    let mut harness = Harness::new(document);

    harness.toggle_inspector();
    let padding_row = harness.row_center(1);
    harness.click(padding_row);
    harness.frame(Vec::new());

    assert_eq!(harness.tree(), ["column", "  frame", "    text"]);

    harness.click(padding_row);
    harness.frame(Vec::new());

    assert_eq!(harness.tree(), ["column", "  frame", "    text"]);
}
