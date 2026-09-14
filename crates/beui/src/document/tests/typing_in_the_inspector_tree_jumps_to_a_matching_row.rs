use super::*;

#[test]
fn typing_in_the_inspector_tree_jumps_to_a_matching_row() {
    let HelloColumn { document, text, .. } = hello_column();
    let mut harness = Harness::new(document);

    harness.toggle_inspector();
    harness.frame(Vec::new());
    harness.toggle_inspector_focus();
    harness.type_text("te");

    assert_eq!(harness.focused_row_index(), Some(2));
    assert_eq!(harness.inspector().state.selected.get(), Some(text));
}
