use super::*;
use block_editor_beui::beui::Modifiers;

#[test]
fn dropping_a_hotbar_slot_on_an_open_folder_puts_it_inside() {
    let mut editor = editor();
    editor.key(Key::Four, Modifiers::NONE);
    editor.run();
    assert!(
        !editor.shown("logic-grid.slot.3.1"),
        "the Logic folder holds one tool"
    );

    let from = editor.rect_of("logic-grid.slot.0").center();
    let to = editor.rect_of("logic-grid.column.1").center();
    editor.drag(from, to);
    editor.run();

    assert!(
        editor.label("logic-grid.slot.0").contains("Merger"),
        "the dragged slot left the top of the hotbar"
    );
    assert!(
        editor.shown("logic-grid.slot.2.1"),
        "the Logic folder moved up a place and holds the dragged slot after its own tool"
    );
    assert!(editor.label("logic-grid.slot.2.1").contains("Wire"));
}
