use super::*;
use block_editor_beui::beui::{Input, PointerButton};

#[test]
fn a_dragged_hotbar_slot_follows_the_pointer() {
    let mut editor = editor();
    let from = editor.rect_of("logic-grid.slot.0").center();
    let to = editor.rect_of("logic-grid.canvas").center();

    editor.step(vec![Input::PointerMoved(from)]);
    editor.step(vec![Input::PointerButton {
        pos: from,
        button: PointerButton::Primary,
        pressed: true,
    }]);
    editor.step(vec![Input::PointerMoved(to)]);
    editor.run();
    let ghost = editor.rect_of("logic-grid.slot-ghost");
    assert!(
        ghost.left() >= to.x && ghost.left() - to.x < 40.0 && ghost.top() >= to.y,
        "the dragged slot is drawn beside the pointer, at {ghost:?}"
    );
    assert!(editor.label("logic-grid.slot-ghost").contains("Wire"));

    editor.step(vec![Input::PointerButton {
        pos: to,
        button: PointerButton::Primary,
        pressed: false,
    }]);
    editor.run();
    assert!(
        !editor.shown("logic-grid.slot-ghost"),
        "the ghost goes with the drop"
    );
    assert!(
        editor.label("logic-grid.slot.0").contains("Wire"),
        "dropping over the canvas leaves the hotbar as it was"
    );
}
