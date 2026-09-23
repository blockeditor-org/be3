use super::*;

#[test]
fn dragging_a_hotbar_slot_onto_another_moves_it_there() {
    let (mut editor, _block) = editor();
    assert!(editor.label("logic-grid.slot.0").contains("Wire"));

    let from = editor.rect_of("logic-grid.slot.0").center();
    let to = editor.rect_of("logic-grid.slot.2").center();
    editor.drag(from, to);
    editor.run();
    editor.run();

    assert!(
        editor.label("logic-grid.slot.0").contains("Merger"),
        "the slot after the dragged one moves up into its place"
    );
    assert!(
        editor.label("logic-grid.slot.2").contains("Wire"),
        "the dragged slot lands where it was dropped"
    );
}
