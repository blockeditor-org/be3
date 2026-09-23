use super::*;

#[test]
fn dropping_a_hotbar_slot_on_an_open_folder_puts_it_inside() {
    let (mut editor, _block) = editor();
    editor.key_press(Key::Four);
    editor.run();
    assert!(
        !editor.shown("logic-grid.slot.3.1"),
        "the Logic folder holds one tool"
    );

    let from = editor.rect_of("logic-grid.slot.0").center();
    let column = editor.rect_of("logic-grid.column.1");
    let to = Pos2::new(column.center().x, column.bottom() - 8.0);
    editor.drag(from, to);
    editor.run();
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
