use super::*;

#[test]
fn ctrl_shift_f_moves_focus_between_the_inspector_and_the_document() {
    let HelloColumn { document, .. } = hello_column();
    let mut harness = Harness::new(document);

    harness.toggle_inspector();
    harness.frame(Vec::new());

    assert_eq!(harness.inspector().document.focused_node(), None);

    harness.toggle_inspector_focus();

    assert!(harness.inspector().document.focused_node().is_some());
    assert_eq!(harness.focused_row_index(), Some(0));
    assert_eq!(harness.document.focused_node(), None);

    harness.toggle_inspector_focus();

    assert_eq!(harness.inspector().document.focused_node(), None);

    harness.toggle_inspector_focus();
    harness.key(Key::Escape, Modifiers::NONE);

    assert_eq!(harness.inspector().document.focused_node(), None);
}
