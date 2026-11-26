use super::*;
use be_sdk_beui::beui::{Input, PointerButton};

#[test]
fn a_slot_carried_across_its_columns_empty_area_still_lands_on_a_slot() {
    let mut editor = editor();
    let from = editor.rect_of("logic-grid.slot.0").center();
    let column = editor.rect_of("logic-grid.column.0");
    let empty = Pos2::new(column.center().x, column.bottom() - 4.0);
    let to = editor.rect_of("logic-grid.slot.2").center();

    editor.step(vec![Input::PointerMoved(from)]);
    editor.step(vec![Input::PointerButton {
        pos: from,
        button: PointerButton::Primary,
        pressed: true,
    }]);
    // Through the column's own area first, where it would go into the folder.
    editor.step(vec![Input::PointerMoved(empty)]);
    editor.run();
    editor.step(vec![Input::PointerMoved(to)]);
    editor.run();
    editor.step(vec![Input::PointerButton {
        pos: to,
        button: PointerButton::Primary,
        pressed: false,
    }]);
    editor.run();

    assert!(
        editor.label("logic-grid.slot.2").contains("Wire"),
        "moved on from the column onto a slot, the slot takes it"
    );
}
